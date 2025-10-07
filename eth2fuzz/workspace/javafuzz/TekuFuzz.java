/*
 * Copyright 2020 Patrick Ventuzelo
 *
 * Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except in compliance with
 * the License. You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on
 * an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the License for the
 * specific language governing permissions and limitations under the License.
 */

import edu.berkeley.cs.jqf.fuzz.Fuzz;
import edu.berkeley.cs.jqf.fuzz.JQF;
import java.io.InputStream;
import org.apache.tuweni.bytes.Bytes;
import tech.pegasys.teku.infrastructure.ssz.sos.SszDeserializeException;
import org.junit.runner.RunWith;
import java.io.IOException;
import tech.pegasys.teku.spec.datastructures.operations.Attestation;
import tech.pegasys.teku.spec.datastructures.operations.AttesterSlashing;
import tech.pegasys.teku.spec.datastructures.blocks.BeaconBlock;
import tech.pegasys.teku.spec.datastructures.blocks.SignedBeaconBlock;
import tech.pegasys.teku.spec.datastructures.operations.Deposit;
import tech.pegasys.teku.spec.datastructures.operations.ProposerSlashing;
import tech.pegasys.teku.spec.datastructures.operations.SignedVoluntaryExit;
import tech.pegasys.teku.spec.datastructures.operations.VoluntaryExit;
import tech.pegasys.teku.spec.datastructures.type.SszSignatureSchema;
import tech.pegasys.teku.bls.BLSSignature;
import java.util.Random;
import java.util.Collections;
import java.util.ArrayList;
import java.util.Arrays;
import java.io.File;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.nio.file.Files;
import tech.pegasys.teku.spec.Spec;
import tech.pegasys.teku.spec.SpecFactory;
import tech.pegasys.teku.spec.schemas.SchemaDefinitions;
import tech.pegasys.teku.infrastructure.unsigned.UInt64;
import java.io.FileInputStream;

/* useful links:
- https://github.com/PegaSysEng/teku/blob/master/ethereum/datastructures/src/main/java/tech/pegasys/teku/datastructures/util/SimpleOffsetSerializer.java
*/

@RunWith(JQF.class)
public class TekuFuzz {
  private static final Spec SPEC = SpecFactory.create("mainnet");

  public static void main(String[] args) {

    // Debug helper is deprecated with new SSZ API; keep placeholder
    System.out.println("[+] TekuFuzz ready");
  }
  // state loading removed in favor of schema-based SSZ-only fuzzing




  // Attestation
  @Fuzz /* JQF will generate inputs to this method */
  public void teku_attestation(InputStream input) {
    try {
      byte[] bytes = input.readAllBytes();
      SchemaDefinitions sd = SPEC.atSlot(UInt64.ZERO).getSchemaDefinitions();
      Attestation a = sd.getAttestationSchema().sszDeserialize(Bytes.wrap(bytes));
    } catch (IOException e) {
    } catch (SszDeserializeException e) {
    } catch (IllegalArgumentException e) {
    }
  }

  // AttesterSlashing
  @Fuzz
  public void teku_attester_slashing(InputStream input) {
    try {
      byte[] bytes = input.readAllBytes();
      SchemaDefinitions sd = SPEC.atSlot(UInt64.ZERO).getSchemaDefinitions();
      AttesterSlashing s = sd.getAttesterSlashingSchema().sszDeserialize(Bytes.wrap(bytes));
    } catch (IOException e) {
    } catch (SszDeserializeException e) {
    } catch (IllegalArgumentException e) {
    }
  }

  // BeaconBlock
  @Fuzz
  public void teku_block(InputStream input) {
  try {
    byte[] bytes = input.readAllBytes();
    SchemaDefinitions sd = SPEC.atSlot(UInt64.ZERO).getSchemaDefinitions();
    SignedBeaconBlock structuredInput = sd.getSignedBeaconBlockSchema().sszDeserialize(Bytes.wrap(bytes));
  } catch (IOException e) {
  } catch (SszDeserializeException e){
  } catch (IllegalStateException e){
  } catch (IllegalArgumentException e){}
  }

  // TODO: SignedVoluntaryExit
  @Fuzz
  public void teku_signed_block(InputStream input) {
  try {
    byte[] bytes = input.readAllBytes();
    SignedVoluntaryExit structuredInput = 
      SignedVoluntaryExit.SSZ_SCHEMA.sszDeserialize(Bytes.wrap(bytes));
  } catch (IOException e) {    
  } catch (SszDeserializeException e){
  } catch (IllegalStateException e){
  } catch (IllegalArgumentException e){}
  }

  // TODO: BeaconBlock
  @Fuzz
  public void teku_block_header(InputStream input) {
    try {
      byte[] bytes = input.readAllBytes();
      SchemaDefinitions sd = SPEC.atSlot(UInt64.ZERO).getSchemaDefinitions();
      BeaconBlock structuredInput = sd.getBeaconBlockSchema().sszDeserialize(Bytes.wrap(bytes));
    } catch (IOException e) {    
    } catch (SszDeserializeException e){
    } catch (IllegalStateException e){
    } catch (IllegalArgumentException e){
    }
  }

  // Deposit
  @Fuzz
  public void teku_deposit(InputStream input) {
    try {
      byte[] bytes = input.readAllBytes();
      Deposit structuredInput = 
       Deposit.SSZ_SCHEMA.sszDeserialize(Bytes.wrap(bytes));

    } catch (IOException e) {    
    } catch (SszDeserializeException e){
    } catch (IllegalStateException e){
    } catch (IllegalArgumentException e){
    }
  }

  // ProposerSlashing
  @Fuzz
  public void teku_proposer_slashing(InputStream input) {
    try {
      byte[] bytes = input.readAllBytes();
      ProposerSlashing structuredInput = 
       ProposerSlashing.SSZ_SCHEMA.sszDeserialize(Bytes.wrap(bytes));

    } catch (IOException e) {    
    } catch (SszDeserializeException e){
    } catch (IllegalStateException e){
    } catch (IllegalArgumentException e){
    }
  }

// TODO: SignedVoluntaryExit
  @Fuzz
  public void teku_signed_voluntary_exit(InputStream input) {
    try {
      byte[] bytes = input.readAllBytes();
      SignedVoluntaryExit structuredInput = 
       SignedVoluntaryExit.SSZ_SCHEMA.sszDeserialize(Bytes.wrap(bytes));

    } catch (IOException e) {    
    } catch (SszDeserializeException e){
    } catch (IllegalStateException e){
    } catch (IllegalArgumentException e){
    }
  }

  // VoluntaryExit
  @Fuzz
  public void teku_voluntary_exit(InputStream input) {
    try {
      byte[] bytes = input.readAllBytes();
      VoluntaryExit structuredInput = 
       VoluntaryExit.SSZ_SCHEMA.sszDeserialize(Bytes.wrap(bytes));


    } catch (IOException e) {    
    } catch (SszDeserializeException e){
    } catch (IllegalStateException e){
    } catch (IllegalArgumentException e){
    }
  }

  // BLSSignature
  @Fuzz
  public void teku_bls(InputStream input) {
  try {
    byte[] bytes = input.readAllBytes();
    BLSSignature structuredInput = 
      SszSignatureSchema.INSTANCE.sszDeserialize(Bytes.wrap(bytes)).getSignature();
  } catch (IOException e) {    
  } catch (SszDeserializeException e){
  } catch (IllegalStateException e){
  } catch (IllegalArgumentException e){}
  }

}



// enr
// https://github.com/PegaSysEng/teku/blob/b5f23a4d2b704713699fdee6e1839b6c2d9dddb6/networking/p2p/src/test/java/tech/pegasys/teku/networking/p2p/discovery/discv5/NodeRecordConverterTest.java#L44
