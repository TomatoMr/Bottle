import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Bottle } from "../target/types/bottle";
import { BN } from "bn.js";
import { sha256 } from "js-sha256";
import bs58 from "bs58";
import { assert } from "chai";
import { PublicKey } from "@solana/web3.js";

describe("bottle", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());
  const program = anchor.workspace.Bottle as Program<Bottle>;
  const bottleDiscriminator = Buffer.from(sha256.digest('account:Bottle')).subarray(0, 8);
  const getADriftingBottle = async (): Promise<any> => {
    // pre-fetch the drifting bottles
    const driftingBottles = await program.provider.connection.getProgramAccounts(
      program.programId,
      {
        dataSlice: {
          offset: 8, // id
          length: 8, // id's length
        },
        filters: [
          { memcmp: { offset: 0, bytes: bs58.encode(bottleDiscriminator) } }, // Ensure it's a Bottle account.
          {
            memcmp: {
              offset: 56, // 8+8+32+8
              bytes: bs58.encode(new Uint8Array(0)), // only fetch drfting bottles
            },
          },
        ],
      },
    );

    const driftingBottlesData = driftingBottles.map(({ pubkey, account }) => ({ pubkey, timestamp: new BN(account.data, 'le') }));
    const sortedBottlesByTimestamp = driftingBottlesData.sort((a, b) => b.timestamp.cmp(a.timestamp));

    if (sortedBottlesByTimestamp.length <= 0) {
      assert.fail("There should have more than 1 bottle.");
    }

    const bottle = await program.account.bottle.fetch(sortedBottlesByTimestamp[0].pubkey);
    return Promise.resolve([bottle, sortedBottlesByTimestamp[0].pubkey]);
  }

  it("can throw a bottle", async () => {
    await program.methods.throwABottle(new BN(Date.now()), new BN(1), "Hello Bottle").accounts({
      sender: program.provider.publicKey,
    }).rpc();
    const bottles = await program.provider.connection.getProgramAccounts(program.programId,
      {
        filters: [
          { memcmp: { offset: 0, bytes: bs58.encode(bottleDiscriminator) } }, // Ensure it's a Bottle account.
        ],
      }
    );
    assert.equal(bottles.length, 1);
  });

  it("throws an error when message is too long", async () => {
    try {
      const message = "A".repeat(401);
      await program.methods.throwABottle(new BN(Date.now()), new BN(1), message).accounts({
        sender: program.provider.publicKey,
      }).rpc();
    } catch (error) {
      assert.equal(error.error.errorMessage, "The message is too long.");
      return;
    }
    assert.fail("The instruction should have failed with a 401-character message.");
  });

  it("throwing away more than three bottles in a day.", async () => {
    try {
      await program.methods.throwABottle(new BN(Date.now()), new BN(2), "My 2nd bottle.").accounts({
        sender: program.provider.publicKey,
      }).rpc();
      await program.methods.throwABottle(new BN(Date.now()), new BN(3), "My 3rd bottle.").accounts({
        sender: program.provider.publicKey,
      }).rpc();
      await program.methods.throwABottle(new BN(Date.now()), new BN(4), "My 4th bottle.").accounts({
        sender: program.provider.publicKey,
      }).rpc();
    } catch (error) {
      assert.equal(error.error.errorMessage, "The maximum number of bottles that can be throwed or retrieved each day has exceeded the limit.");
      return;
    }
    assert.fail("The last transction should have failed with more than 3 bottles.");
  });

  it("retrieve a bottle by the same person", async () => {
    try {
      const [bottle, bottlePubkey] = await getADriftingBottle();
      await program.methods.retrieveABottle().accounts({
        bottle: bottlePubkey,
        retrievee: program.provider.publicKey,
        bottleAsset: bottle.assetAccount,
      }).rpc();
    } catch (error) {
      assert.equal(error.error.errorMessage, "The same person cannot retrieve their own bottle.");
      return;
    }
    assert.fail("The bottle is retrieved by the same person, it should have failed.");
  });

  // TODO: theOtherPerson need a wallet
  // it("retrieve a bottle by the other person", async () => {
  //   const [bottle, bottlePubkey] = await getADriftingBottle();
  //   await program.methods.retrieveABottle().accounts({
  //     bottle: bottlePubkey,
  //     retrievee: theOtherPerson,
  //     bottleAsset: bottle.assetAccount,
  //   }).rpc();
    
  //   const drftingBottles = await program.provider.connection.getProgramAccounts(program.programId,
  //     {
  //       filters: [
  //         { memcmp: { offset: 0, bytes: bs58.encode(bottleDiscriminator) } }, // Ensure it's a Bottle account.
  //         {
  //           memcmp: {
  //             offset: 56, // 8+8+32+8
  //             bytes: bs58.encode(new Uint8Array(0)), // only fetch drfting bottles
  //           },
  //         },
  //       ],
  //     }
  //   );
  //   const retrievedBottles = await program.provider.connection.getProgramAccounts(program.programId,
  //     {
  //       filters: [
  //         { memcmp: { offset: 0, bytes: bs58.encode(bottleDiscriminator) } }, // Ensure it's a Bottle account.
  //         {
  //           memcmp: {
  //             offset: 56, // 8+8+32+8
  //             bytes: bs58.encode(new Uint8Array(1)), // only fetch retrieved bottles
  //           },
  //         },
  //       ],
  //     }
  //   );

  //   assert.equal(drftingBottles.length, 2);
  //   assert.equal(retrievedBottles.length, 1);
  // });

  // it("retrieve more than 3 bottles in a day", async () => {
  //   // 2nd bottle
  //   const [bottle2nd, bottlePubkey2nd] = await getADriftingBottle();
  //   await program.methods.retrieveABottle().accounts({
  //     bottle: bottlePubkey2nd,
  //     retrievee: theOtherPerson,
  //     bottleAsset: bottle2nd.assetAccount,
  //   }).rpc();

  //   // 3rd bottle
  //   const [bottle3rd, bottlePubkey3rd] = await getADriftingBottle();
  //   await program.methods.retrieveABottle().accounts({
  //     bottle: bottlePubkey3rd,
  //     retrievee: theOtherPerson,
  //     bottleAsset: bottle3rd.assetAccount,
  //   }).rpc();
  //   try {
  //     // 4th bottle
  //     const [bottle4th, bottlePubkey4th] = await getADriftingBottle();
  //     await program.methods.retrieveABottle().accounts({
  //       bottle: bottlePubkey4th,
  //       retrievee: theOtherPerson,
  //       bottleAsset: bottle4th.assetAccount,
  //     }).rpc();
  //   } catch (error) {
  //     assert.equal(error.error.errorMessage, "The same person cannot retrieve their own bottle.");
  //     return;
  //   }
  //   assert.fail("The bottle");
  // });
})