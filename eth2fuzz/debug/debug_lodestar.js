const { ENR } = require("@chainsafe/enr");

buf = Buffer.from('XXX', 'hex').toString()
// buf = Buffer.from('XXX', 'hex')

console.log(buf)

ENR.decodeTxt(buf);
