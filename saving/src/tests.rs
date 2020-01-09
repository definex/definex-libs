#![cfg(test)]
#![allow(dead_code)]

use super::*;
use support::{assert_noop, assert_ok};

#[allow(unused_imports)]
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, Dispatchable, IdentityLookup, OnFinalize, OnInitialize},
    Perbill,
};

use crate::mock::{constants::*, new_test_ext, Call, ExtBuilder, Origin, SavingTest, TestRuntime};

#[test]
fn genesis_values() {
    ExtBuilder::default().build().execute_with(|| {
        assert_eq!(SavingTest::current_phase_id(), PHASE1);
        assert_eq!(
            SavingTest::phase_info(PHASE2),
            PhaseInfo {
                id: PHASE2,
                quota: PHASE2_QUOTA,
                exchange: PHASE2_EXCHANGE,
                iou_asset_id: Some(RSC2_ASSET_ID),
            }
        );
    });
}

#[test]
fn current_phase_id() {
    ExtBuilder::default().build().execute_with(|| {
        assert_eq!(SavingTest::current_phase_id(), PHASE1);
    });
}

#[test]
fn set_iou_asset_id() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(SavingTest::set_iou_asset_id_for_phase(
            Origin::ROOT,
            PHASE1,
            RSC5_ASSET_ID
        ));
        assert_eq!(
            SavingTest::phase_info(PHASE1).iou_asset_id,
            Some(RSC5_ASSET_ID)
        );
    });
}

#[test]
fn create_staking() {
    ExtBuilder::default().build().execute_with(|| {
        assert_noop!(SavingTest::create_staking(ALICE, 0), "saving can't be zero");
        assert_ok!(SavingTest::create_staking(ALICE, 10 * DECIMALS));
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RSC1_ASSET_ID, &ALICE),
            10 * DECIMALS
        );
    });
}

#[test]
fn staking_by_transfer() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(<assets::Module<TestRuntime>>::mint(
            Origin::ROOT,
            SBTC_ASSET_ID,
            ALICE,
            100 * DECIMALS
        ));
        assert_ok!(<assets::Module<TestRuntime>>::transfer(
            Origin::signed(ALICE),
            SBTC_ASSET_ID,
            COLLECTION_ACCOUNT_ID,
            10 * DECIMALS
        ));
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&SBTC_ASSET_ID, &ALICE),
            90 * DECIMALS
        );
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RSC1_ASSET_ID, &ALICE),
            10 * DECIMALS
        );
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RBTC_ASSET_ID, &ALICE),
            20000 * DECIMALS
        );
        assert_ok!(<assets::Module<TestRuntime>>::transfer(
            Origin::signed(ALICE),
            SBTC_ASSET_ID,
            COLLECTION_ACCOUNT_ID,
            10 * DECIMALS
        ));
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RSC1_ASSET_ID, &ALICE),
            20 * DECIMALS
        );
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RBTC_ASSET_ID, &ALICE),
            40000 * DECIMALS
        );
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&SBTC_ASSET_ID, &ALICE),
            80 * DECIMALS
        );
    });
}

#[test]
fn staking_by_staking() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(<assets::Module<TestRuntime>>::mint(
            Origin::ROOT,
            SBTC_ASSET_ID,
            ALICE,
            100 * DECIMALS
        ));
        assert_ok!(SavingTest::staking(
            Origin::signed(ALICE),
            SBTC_ASSET_ID,
            10 * DECIMALS
        ));
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&SBTC_ASSET_ID, &ALICE),
            90 * DECIMALS
        );
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RSC1_ASSET_ID, &ALICE),
            10 * DECIMALS
        );
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RBTC_ASSET_ID, &ALICE),
            20000 * DECIMALS
        );
        assert_ok!(SavingTest::sudo_staking(
            Origin::ROOT,
            SBTC_ASSET_ID,
            10 * DECIMALS,
            ALICE
        ));
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RSC1_ASSET_ID, &ALICE),
            20 * DECIMALS
        );
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RBTC_ASSET_ID, &ALICE),
            40000 * DECIMALS
        );
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&SBTC_ASSET_ID, &ALICE),
            80 * DECIMALS
        );
    });
}

#[test]
fn iou_when_create_staking() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(SavingTest::create_staking(ALICE, 1 * DECIMALS));
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RSC1_ASSET_ID, &ALICE),
            1 * DECIMALS
        );

        let bob_saving = 1_1000_0000;
        assert_ok!(SavingTest::create_staking(BOB, bob_saving));
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RSC1_ASSET_ID, &BOB),
            bob_saving
        );
    });
}

#[test]
fn share_when_create_staking() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(SavingTest::create_staking(ALICE, 1 * DECIMALS));
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RBTC_ASSET_ID, &ALICE),
            PHASE1_EXCHANGE * DECIMALS / NUM_OF_PHASE as u128
        );
        let alice_release_list = SavingTest::account_future_releases(&ALICE);
        assert_eq!(alice_release_list.len(), 1);
        assert_eq!(alice_release_list[0].get_total_balance(), 8000_00000000);
    });
}

#[test]
fn iou_with_shift_phases() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(SavingTest::create_staking(ALICE, 200 * DECIMALS));
        assert_eq!(SavingTest::current_phase_id(), PHASE2);
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RSC1_ASSET_ID, &ALICE),
            PHASE1_QUOTA
        );
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RSC2_ASSET_ID, &ALICE),
            200 * DECIMALS - PHASE1_QUOTA
        );
    });
}

#[test]
fn share_with_shift_phases() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(SavingTest::create_staking(ALICE, 200 * DECIMALS));
        assert_eq!(SavingTest::current_phase_id(), PHASE2);
        assert_eq!(SavingTest::num_of_phases_left(), 4);
        let phase1_portion = PHASE1_QUOTA * PHASE1_EXCHANGE / NUM_OF_PHASE as u128;
        let phase2_portion =
            (200 * DECIMALS - PHASE1_QUOTA) * PHASE2_EXCHANGE / (NUM_OF_PHASE - 1) as u128;
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RBTC_ASSET_ID, &ALICE),
            phase1_portion + phase1_portion + phase2_portion
        );
        assert_ok!(SavingTest::create_staking(ALICE, 500 * DECIMALS));
        assert_eq!(SavingTest::current_phase_id(), PHASE3);
        assert_eq!(SavingTest::num_of_phases_left(), 3);
        let phase2_portion_se = (PHASE2_QUOTA + PHASE1_QUOTA - 200 * DECIMALS) * PHASE2_EXCHANGE
            / (NUM_OF_PHASE - 1) as u128;
        let phase3_portion = (((500 + 200) * DECIMALS - PHASE2_QUOTA - PHASE1_QUOTA)
            * PHASE3_EXCHANGE
            + (NUM_OF_PHASE - 3) as u128)
            / (NUM_OF_PHASE - 2) as u128;
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RBTC_ASSET_ID, &ALICE),
            phase1_portion
                + phase1_portion
                + phase1_portion
                + phase2_portion
                + phase2_portion
                + phase2_portion_se
                + phase2_portion_se
                + phase3_portion
        );
    });
}

#[test]
fn once_for_all_quota() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(SavingTest::create_staking(
            ALICE,
            PHASE1_QUOTA + PHASE2_QUOTA + PHASE3_QUOTA + PHASE4_QUOTA + PHASE5_QUOTA
        ));
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RBTC_ASSET_ID, &ALICE),
            PHASE1_QUOTA * PHASE1_EXCHANGE
                + PHASE2_QUOTA * PHASE2_EXCHANGE
                + PHASE3_QUOTA * PHASE3_EXCHANGE
                + PHASE4_QUOTA * PHASE4_EXCHANGE
                + PHASE5_QUOTA * PHASE5_EXCHANGE
        );
        let alice_releases = SavingTest::account_future_releases(ALICE);
        assert_eq!(alice_releases.len(), 0);
    });
}

#[test]
fn wire_sbtc_to_collection_account() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(<assets::Module<TestRuntime>>::mint(
            Origin::ROOT,
            SBTC_ASSET_ID,
            ALICE,
            1_00000000
        ));
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&SBTC_ASSET_ID, &ALICE),
            1_00000000
        );
        assert_ok!(<assets::Module<TestRuntime>>::transfer(
            Origin::signed(ALICE),
            SBTC_ASSET_ID,
            COLLECTION_ACCOUNT_ID,
            1_00000000
        ));
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RSC1_ASSET_ID, &ALICE),
            1_00000000
        );
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RBTC_ASSET_ID, &ALICE),
            2000_00000000
        );
    });
}

#[test]
fn redeem_1_sbtc_by_transfer() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(<assets::Module<TestRuntime>>::mint(
            Origin::ROOT,
            SBTC_ASSET_ID,
            ALICE,
            1_00000000
        ));
        assert_ok!(<assets::Module<TestRuntime>>::transfer(
            Origin::signed(ALICE),
            SBTC_ASSET_ID,
            COLLECTION_ACCOUNT_ID,
            1_00000000
        ));
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RSC1_ASSET_ID, &ALICE),
            1_00000000
        );
        assert_ok!(<assets::Module<TestRuntime>>::transfer(
            Origin::signed(ALICE),
            RSC1_ASSET_ID,
            COLLECTION_ACCOUNT_ID,
            5000_0000
        ));
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RSC1_ASSET_ID, &ALICE),
            5000_0000
        );
        let alice_release = SavingTest::account_future_releases(ALICE);
        assert_eq!(alice_release.len(), 1);
        assert_eq!(alice_release[0].major.per_term, 1000_0000_0000);
        assert_eq!(alice_release[0].major.terms_left, 4);
        assert_eq!(alice_release[0].major.terms_total, 5);
        assert_eq!(alice_release[0].minor.is_none(), true);
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RBTC_ASSET_ID, &ALICE,),
            1000_00000000
        );
        assert_ok!(<assets::Module<TestRuntime>>::transfer(
            Origin::signed(ALICE),
            RSC1_ASSET_ID,
            COLLECTION_ACCOUNT_ID,
            5000_0000
        ));
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&SBTC_ASSET_ID, &ALICE),
            1_00000000
        );
        assert_eq!(SavingTest::share_asset_collected(&ALICE), 0);
    });
}

#[test]
fn redeem_501_sbtc_by_redeem() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(<assets::Module<TestRuntime>>::mint(
            Origin::ROOT,
            SBTC_ASSET_ID,
            BOB,
            501_00000000
        ));
        assert_ok!(<assets::Module<TestRuntime>>::transfer(
            Origin::signed(BOB),
            SBTC_ASSET_ID,
            COLLECTION_ACCOUNT_ID,
            1_00000000
        ));
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RSC1_ASSET_ID, &BOB),
            1_00000000
        );
        assert_ok!(SavingTest::redeem(
            Origin::signed(BOB),
            RSC1_ASSET_ID,
            10000_0000
        ));
        assert_ok!(SavingTest::staking(
            Origin::signed(BOB),
            SBTC_ASSET_ID,
            500_00000000
        ));

        assert_ok!(SavingTest::redeem(
            Origin::signed(BOB),
            RSC2_ASSET_ID,
            400_00000000
        ));
        assert_ok!(SavingTest::redeem(
            Origin::signed(BOB),
            RSC1_ASSET_ID,
            99_00000000
        ));

        assert_eq!(SavingTest::current_phase_id(), 3);
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RSC3_ASSET_ID, &BOB),
            10000_0000
        );
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RBTC_ASSET_ID, &BOB),
            1666_6666_6667
        );

        assert_ok!(SavingTest::redeem(
            Origin::signed(BOB),
            RSC3_ASSET_ID,
            8000_0000
        ));

        assert_ok!(SavingTest::redeem(
            Origin::signed(BOB),
            RSC3_ASSET_ID,
            1300_0000
        ));
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RSC3_ASSET_ID, &BOB),
            700_0000
        );

        assert_ok!(SavingTest::redeem(
            Origin::signed(BOB),
            RSC3_ASSET_ID,
            111_0000
        ));
        assert_ok!(SavingTest::redeem(
            Origin::signed(BOB),
            RSC3_ASSET_ID,
            589_0000
        ));
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RBTC_ASSET_ID, &BOB),
            0
        );
    });
}

#[test]
fn redeem_1_sbtc_by_redeem() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(<assets::Module<TestRuntime>>::mint(
            Origin::ROOT,
            SBTC_ASSET_ID,
            ALICE,
            1_00000000
        ));
        assert_ok!(<assets::Module<TestRuntime>>::transfer(
            Origin::signed(ALICE),
            SBTC_ASSET_ID,
            COLLECTION_ACCOUNT_ID,
            1_00000000
        ));
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RSC1_ASSET_ID, &ALICE),
            1_00000000
        );
        assert_ok!(SavingTest::redeem(
            Origin::signed(ALICE),
            RSC1_ASSET_ID,
            5000_0000
        ));
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RSC1_ASSET_ID, &ALICE),
            5000_0000
        );
        let alice_release = SavingTest::account_future_releases(ALICE);
        assert_eq!(alice_release.len(), 1);
        assert_eq!(alice_release[0].major.per_term, 1000_0000_0000);
        assert_eq!(alice_release[0].major.terms_left, 4);
        assert_eq!(alice_release[0].major.terms_total, 5);
        assert_eq!(alice_release[0].minor.is_none(), true);
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RBTC_ASSET_ID, &ALICE,),
            1000_00000000
        );
        assert_ok!(SavingTest::redeem(
            Origin::signed(ALICE),
            RSC1_ASSET_ID,
            5000_0000
        ));
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&SBTC_ASSET_ID, &ALICE),
            1_00000000
        );
        assert_eq!(SavingTest::share_asset_collected(&ALICE), 0);
    });
}

#[test]
fn illegal_redeem() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(<assets::Module<TestRuntime>>::mint(
            Origin::ROOT,
            SBTC_ASSET_ID,
            ALICE,
            1_00000000
        ));
        assert_ok!(<assets::Module<TestRuntime>>::transfer(
            Origin::signed(ALICE),
            SBTC_ASSET_ID,
            COLLECTION_ACCOUNT_ID,
            1_00000000
        ));
        assert_noop!(
            <assets::Module<TestRuntime>>::transfer(
                Origin::signed(ALICE),
                RSC1_ASSET_ID,
                COLLECTION_ACCOUNT_ID,
                1_10000000
            ),
            "not enough available share asset"
        );
        assert_ok!(<assets::Module<TestRuntime>>::mint(
            Origin::ROOT,
            RSC1_ASSET_ID,
            ALICE,
            1_00000000
        ));
        assert_noop!(
            <assets::Module<TestRuntime>>::transfer(
                Origin::signed(ALICE),
                RSC1_ASSET_ID,
                COLLECTION_ACCOUNT_ID,
                1_10000000
            ),
            "not enough available share asset"
        );
        assert_noop!(
            <assets::Module<TestRuntime>>::transfer(
                Origin::signed(ALICE),
                RBTC_ASSET_ID,
                COLLECTION_ACCOUNT_ID,
                5000_00000000
            ),
            "balance too low to send amount"
        );
    });
}

#[test]
fn shares_release_list() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(SavingTest::create_staking(ALICE, PHASE1_QUOTA / 2,));
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RBTC_ASSET_ID, &ALICE),
            PHASE1_QUOTA / 10 * PHASE1_EXCHANGE
        );
        let alice_releases = SavingTest::account_future_releases(ALICE);
        assert_eq!(alice_releases.len(), 1);
        assert_eq!(alice_releases[0].major.terms_left, 4);

        assert_ok!(SavingTest::create_staking(BOB, PHASE1_QUOTA / 2,));
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&RBTC_ASSET_ID, &BOB),
            PHASE1_QUOTA / 5 * PHASE1_EXCHANGE
        );
        let bob_releases = SavingTest::account_future_releases(BOB);
        assert_eq!(bob_releases.len(), 1);
        assert_eq!(bob_releases[0].major.terms_left, 3);

        let alice_releases = SavingTest::account_future_releases(ALICE);
        assert_eq!(alice_releases.len(), 1);
        assert_eq!(alice_releases[0].major.terms_left, 3);

        assert_ok!(SavingTest::create_staking(BOB, PHASE2_QUOTA / 2,));
        let bob_releases = SavingTest::account_future_releases(BOB);
        assert_eq!(bob_releases.len(), 2);
        assert_eq!(bob_releases[0].major.terms_left, 3);
        assert_eq!(bob_releases[1].major.terms_left, 3);
        let alice_releases = SavingTest::account_future_releases(ALICE);
        assert_eq!(alice_releases.len(), 1);
        assert_eq!(alice_releases[0].major.terms_left, 3);
    });
}

#[test]
fn pause_works() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(SavingTest::pause(Origin::ROOT));
        assert_eq!(SavingTest::paused(), true);

        assert_ok!(<assets::Module<TestRuntime>>::mint(
            Origin::ROOT,
            SBTC_ASSET_ID,
            ALICE,
            100 * DECIMALS
        ));
        assert_noop!(
            SavingTest::staking(Origin::signed(ALICE), SBTC_ASSET_ID, 10 * DECIMALS),
            "module is paused"
        );
        assert_noop!(
            SavingTest::redeem(Origin::signed(ALICE), RSC1_ASSET_ID, 5000_0000),
            "module is paused"
        );
        assert_ok!(SavingTest::resume(Origin::ROOT));
        assert_eq!(SavingTest::paused(), false);
    });
}

#[test]
fn share_asset_distribution() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(SavingTest::create_staking(CHRIS, 10_00000000));
        assert_ok!(SavingTest::create_staking(BOB, 10_00000000));
        assert_eq!(
            SavingTest::account_shares(CHRIS),
            10_00000000 / (NUM_OF_PHASE as u128) * PHASE1_EXCHANGE
        );
        assert_eq!(
            SavingTest::account_shares(BOB),
            10_00000000 / (NUM_OF_PHASE as u128) * PHASE1_EXCHANGE
        );

        assert_ok!(<assets::Module<TestRuntime>>::transfer(
            Origin::signed(CHRIS),
            RBTC_ASSET_ID,
            BOB,
            1_0000_00000000
        ));
        assert_eq!(SavingTest::account_shares(CHRIS), 1_0000_00000000);
        assert_eq!(SavingTest::account_shares(BOB), 3_0000_00000000);
        assert_ok!(<assets::Module<TestRuntime>>::transfer(
            Origin::signed(CHRIS),
            RBTC_ASSET_ID,
            DAVE,
            1_0000_00000000
        ));
        assert_eq!(SavingTest::account_shares(CHRIS), 0);
        assert_eq!(SavingTest::account_shares(DAVE), 1_0000_00000000);
    });
}

// this is for future reference
#[test]
fn deposit_event_should_work() {
    ExtBuilder::default().build().execute_with(|| {
        // System::initialize(
        //     &1,
        //     &[0u8; 32].into(),
        //     &[0u8; 32].into(),
        //     &Default::default(),
        // );
        // System::note_finished_extrinsics();
        // System::deposit_event(1u16);
        // System::finalize();
        // assert_eq!(
        //     System::events(),
        //     vec![EventRecord {
        //         phase: Phase::Finalization,
        //         event: 1u16,
        //         topics: vec![],
        //     }]
        // );

        // System::initialize(
        //     &2,
        //     &[0u8; 32].into(),
        //     &[0u8; 32].into(),
        //     &Default::default(),
        // );
        // System::deposit_event(42u16);
        // System::note_applied_extrinsic(&Ok(()), 0);
        // System::note_applied_extrinsic(&Err(DispatchError::new(Some(1), 2, None)), 0);
        // System::note_finished_extrinsics();
        // System::deposit_event(3u16);
        // System::finalize();
        // assert_eq!(
        //     System::events(),
        //     vec![
        //         EventRecord {
        //             phase: Phase::ApplyExtrinsic(0),
        //             event: 42u16,
        //             topics: vec![]
        //         },
        //         EventRecord {
        //             phase: Phase::ApplyExtrinsic(0),
        //             event: 100u16,
        //             topics: vec![]
        //         },
        //         EventRecord {
        //             phase: Phase::ApplyExtrinsic(1),
        //             event: 101u16,
        //             topics: vec![]
        //         },
        //         EventRecord {
        //             phase: Phase::Finalization,
        //             event: 3u16,
        //             topics: vec![]
        //         }
        //     ]
        // );
    });
}

#[test]
fn manually_release_bonus() {
    ExtBuilder::default().build().execute_with(|| {
        assert_eq!(SavingTest::profit_asset_id(), TBD_ASSET_ID);
        assert_eq!(SavingTest::share_asset_id(), RBTC_ASSET_ID);
        assert_ok!(<assets::Module<TestRuntime>>::mint(
            Origin::ROOT,
            RBTC_ASSET_ID,
            DAVE,
            10000_0000
        ));
        assert_ok!(<assets::Module<TestRuntime>>::mint(
            Origin::ROOT,
            RBTC_ASSET_ID,
            BOB,
            10000_0000
        ));
        assert_ok!(<assets::Module<TestRuntime>>::mint(
            Origin::ROOT,
            RBTC_ASSET_ID,
            CHRIS,
            80000_0000
        ));
        assert_eq!(SavingTest::account_shares(&DAVE), 100000000);
        assert_eq!(SavingTest::account_shares(&BOB), 100000000);
        assert_eq!(SavingTest::account_shares(&CHRIS), 800000000);
        assert_eq!(SavingTest::shares_circulation(), 1000000000);
        assert_ok!(<assets::Module<TestRuntime>>::mint(
            Origin::ROOT,
            TBD_ASSET_ID,
            PROFIT_POOL,
            2400000000,
        ));

        SavingTest::dispatch_bonus();

        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&TBD_ASSET_ID, &TEAM),
            480000000
        );
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&TBD_ASSET_ID, &DAVE),
            192000000
        );
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&TBD_ASSET_ID, &BOB),
            192000000
        );
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&TBD_ASSET_ID, &CHRIS),
            192000000 * 8
        );
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&TBD_ASSET_ID, &PROFIT_POOL),
            0
        );
    });
}

/// this is our ultimate test example
#[test]
fn save_12000_sbtc() {
    ExtBuilder::default().build().execute_with(|| {
        // this actually works fine
        // let root_origin = system::RawOrigin::Root;
        // let dispatchable = <assets::Module<TestRuntime>>::mint(root_origin.into(), SBTC_ASSET_ID, DAVE, 100_00000000);

        // but we wanna dig a little further
        let root_origin = Origin::signed(<sudo::Module<TestRuntime>>::key());
        let dispatchable = assets::Call::<TestRuntime>::mint(SBTC_ASSET_ID, DAVE, 100_00000000);
        let proposal = Box::new(Call::Assets(dispatchable));
        sudo::Call::<TestRuntime>::sudo(proposal)
            .dispatch(root_origin)
            .unwrap();
        assert_eq!(
            <assets::Module<TestRuntime>>::free_balance(&SBTC_ASSET_ID, &DAVE),
            100_00000000
        );
    });
}
