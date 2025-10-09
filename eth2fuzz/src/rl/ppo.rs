use failure::Error;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct PPOPolicy {
    pub input_dim: usize,
    pub hidden: usize,
    pub w1: Vec<f64>, // [hidden * input_dim]
    pub b1: Vec<f64>, // [hidden]
    pub w2: Vec<f64>, // [hidden]
    pub b2: f64,      // scalar
    pub lr: f64,
    pub clip_eps: f64,
    pub baseline_ewma: f64,
    pub baseline_beta: f64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Transition {
    pub xs: Vec<Vec<f64>>,   // features per arm at decision
    pub old_probs: Vec<f64>, // probs per arm before update
    pub chosen: usize,       // chosen arm index
    pub reward: f64,         // scalar reward
}

impl PPOPolicy {
    pub fn new(input_dim: usize, hidden: usize, lr: f64, clip_eps: f64) -> Self {
        // Xavier-like init
        let mut w1 = vec![0.0; hidden * input_dim];
        let mut b1 = vec![0.0; hidden];
        let mut w2 = vec![0.0; hidden];
        let b2 = 0.0;
        let scale1 = (2.0 / (input_dim as f64)).sqrt();
        let scale2 = (2.0 / (hidden as f64)).sqrt();
        // deterministic init without RNG
        for i in 0..(hidden * input_dim) {
            let v = ((i as f64 * 12.9898).sin() * 43758.5453).fract() * 2.0 - 1.0;
            w1[i] = v * scale1 * 0.1;
        }
        for i in 0..hidden {
            let v = ((i as f64 * 78.233).sin() * 12345.6789).fract() * 2.0 - 1.0;
            w2[i] = v * scale2 * 0.1;
        }
        Self {
            input_dim,
            hidden,
            w1,
            b1,
            w2,
            b2,
            lr,
            clip_eps,
            baseline_ewma: 0.0,
            baseline_beta: 0.9,
        }
    }

    fn relu(x: f64) -> f64 {
        if x > 0.0 {
            x
        } else {
            0.0
        }
    }
    fn relu_grad(x: f64) -> f64 {
        if x > 0.0 {
            1.0
        } else {
            0.0
        }
    }

    // Forward for one feature vector: returns (hidden pre-activation, hidden, logit)
    fn forward_one(&self, x: &[f64]) -> (Vec<f64>, Vec<f64>, f64) {
        let mut z1 = vec![0.0; self.hidden];
        for h in 0..self.hidden {
            let mut s = self.b1[h];
            let base = h * self.input_dim;
            for d in 0..self.input_dim {
                s += self.w1[base + d] * x[d];
            }
            z1[h] = s;
        }
        let mut h1 = vec![0.0; self.hidden];
        for h in 0..self.hidden {
            h1[h] = Self::relu(z1[h]);
        }
        let mut z2 = self.b2;
        for h in 0..self.hidden {
            z2 += self.w2[h] * h1[h];
        }
        (z1, h1, z2)
    }

    pub fn policy_logits(&self, xs: &[Vec<f64>]) -> (Vec<Vec<f64>>, Vec<Vec<f64>>, Vec<f64>) {
        // returns (z1_list, h1_list, logits)
        let mut z1s = Vec::with_capacity(xs.len());
        let mut hs = Vec::with_capacity(xs.len());
        let mut logits = Vec::with_capacity(xs.len());
        for x in xs.iter() {
            let (z1, h1, z) = self.forward_one(x);
            z1s.push(z1);
            hs.push(h1);
            logits.push(z);
        }
        (z1s, hs, logits)
    }

    pub fn softmax(logits: &[f64]) -> Vec<f64> {
        let maxv = logits.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let exps: Vec<f64> = logits.iter().map(|z| (z - maxv).exp()).collect();
        let sum: f64 = exps.iter().sum();
        if sum <= 0.0 {
            return vec![1.0 / logits.len() as f64; logits.len()];
        }
        exps.iter().map(|e| e / sum).collect()
    }

    pub fn act(&self, xs: &[Vec<f64>]) -> (usize, Vec<f64>) {
        let (_z1s, _hs, logits) = self.policy_logits(xs);
        let probs = Self::softmax(&logits);
        // greedy for determinism in CI; stochastic could be added
        let mut best = 0usize;
        let mut bestp = probs[0];
        for i in 1..probs.len() {
            if probs[i] > bestp {
                best = i;
                bestp = probs[i];
            }
        }
        (best, probs)
    }

    pub fn update_from_transitions(&mut self, batch: &[Transition]) -> Result<(), Error> {
        if batch.is_empty() {
            return Ok(());
        }
        // Accumulate gradients
        let mut g_w1 = vec![0.0; self.hidden * self.input_dim];
        let mut g_b1 = vec![0.0; self.hidden];
        let mut g_w2 = vec![0.0; self.hidden];
        let mut g_b2 = 0.0f64;

        for tr in batch.iter() {
            let (z1s, hs, logits) = self.policy_logits(&tr.xs);
            let probs = Self::softmax(&logits);
            let p_new = probs[tr.chosen].max(1e-8);
            let p_old = tr.old_probs[tr.chosen].max(1e-8);
            let ratio = p_new / p_old;
            // advantage via EWMA baseline
            let adv = tr.reward - self.baseline_ewma;
            // PPO clipping: if ratio outside [1-eps, 1+eps], skip update (zero grad)
            if ratio < (1.0 - self.clip_eps) || ratio > (1.0 + self.clip_eps) {
                // update baseline only
                self.baseline_ewma = self.baseline_beta * self.baseline_ewma
                    + (1.0 - self.baseline_beta) * tr.reward;
                continue;
            }

            // dL/dlogits_j = (p_j - 1{j=a}) * (-adv)
            let mut dL_dz: Vec<f64> = Vec::with_capacity(probs.len());
            for (j, &p) in probs.iter().enumerate() {
                let v = if j == tr.chosen {
                    (p - 1.0) * (-adv)
                } else {
                    p * (-adv)
                };
                dL_dz.push(v);
            }

            // Accumulate grads per arm
            for j in 0..tr.xs.len() {
                let x = &tr.xs[j];
                let z1 = &z1s[j];
                let h1 = &hs[j];
                let dz = dL_dz[j];

                // w2, b2 grads
                for h in 0..self.hidden {
                    g_w2[h] += dz * h1[h];
                }
                g_b2 += dz;

                // backprop to h1
                let mut dL_dh1 = vec![0.0; self.hidden];
                for h in 0..self.hidden {
                    dL_dh1[h] = dz * self.w2[h];
                }
                // relu grad
                let mut dL_dz1 = vec![0.0; self.hidden];
                for h in 0..self.hidden {
                    dL_dz1[h] = dL_dh1[h] * Self::relu_grad(z1[h]);
                }
                // w1, b1 grads
                for h in 0..self.hidden {
                    let base = h * self.input_dim;
                    for d in 0..self.input_dim {
                        g_w1[base + d] += dL_dz1[h] * x[d];
                    }
                    g_b1[h] += dL_dz1[h];
                }
            }

            // Baseline update
            self.baseline_ewma =
                self.baseline_beta * self.baseline_ewma + (1.0 - self.baseline_beta) * tr.reward;
        }

        // Apply gradients (SGD)
        let scale = self.lr / (batch.len() as f64);
        for i in 0..self.w1.len() {
            self.w1[i] -= scale * g_w1[i];
        }
        for i in 0..self.b1.len() {
            self.b1[i] -= scale * g_b1[i];
        }
        for i in 0..self.w2.len() {
            self.w2[i] -= scale * g_w2[i];
        }
        self.b2 -= scale * g_b2;
        Ok(())
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "input_dim": self.input_dim,
            "hidden": self.hidden,
            "w1": self.w1,
            "b1": self.b1,
            "w2": self.w2,
            "b2": self.b2,
            "lr": self.lr,
            "clip_eps": self.clip_eps,
            "baseline_ewma": self.baseline_ewma,
            "baseline_beta": self.baseline_beta,
        })
    }

    pub fn from_json(v: &serde_json::Value) -> Option<Self> {
        let input_dim = v.get("input_dim")?.as_u64()? as usize;
        let hidden = v.get("hidden")?.as_u64()? as usize;
        let w1 = v
            .get("w1")?
            .as_array()?
            .iter()
            .filter_map(|x| x.as_f64())
            .collect::<Vec<_>>();
        let b1 = v
            .get("b1")?
            .as_array()?
            .iter()
            .filter_map(|x| x.as_f64())
            .collect::<Vec<_>>();
        let w2 = v
            .get("w2")?
            .as_array()?
            .iter()
            .filter_map(|x| x.as_f64())
            .collect::<Vec<_>>();
        let b2 = v.get("b2")?.as_f64()?;
        let lr = v.get("lr")?.as_f64()?;
        let clip_eps = v.get("clip_eps")?.as_f64()?;
        let baseline_ewma = v.get("baseline_ewma")?.as_f64()?;
        let baseline_beta = v.get("baseline_beta")?.as_f64()?;
        Some(Self {
            input_dim,
            hidden,
            w1,
            b1,
            w2,
            b2,
            lr,
            clip_eps,
            baseline_ewma,
            baseline_beta,
        })
    }
}
