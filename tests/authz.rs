use osmosis_test_tube::{
    fn_execute, fn_query, Account, Bank, Gamm, Module, OsmosisTestApp, Runner,
    RunnerError::QueryError, SigningAccount,
};

use cosmwasm_std::Coin;
use osmosis_std::shim::{Any, Timestamp as OsmosisTimestamp};
use osmosis_std::types::cosmos::authz::v1beta1::{
    Grant, GrantAuthorization, MsgExec, MsgExecResponse, MsgGrant, MsgGrantResponse,
    QueryGranteeGrantsRequest, QueryGranteeGrantsResponse, QueryGranterGrantsRequest,
    QueryGranterGrantsResponse, QueryGrantsRequest, QueryGrantsResponse,
};

use osmosis_std::types::cosmos::bank::v1beta1::{
    QueryAllBalancesRequest, QueryAllBalancesResponse, QueryBalanceRequest,
};
use osmosis_std::types::osmosis::gamm::v1beta1::QueryTotalSharesRequest;
use osmosis_std::types::osmosis::incentives::{Gauge, GaugesRequest, GaugesResponse};
use osmosis_std::types::osmosis::lockup::{
    AccountLockedCoinsRequest, AccountLockedCoinsResponse, MsgLockTokens, MsgLockTokensResponse,
};
use osmosis_std::types::osmosis::poolincentives::v1beta1::{
    DistrRecord, QueryGaugeIdsRequest, QueryGaugeIdsResponse, ReplacePoolIncentivesProposal,
    UpdatePoolIncentivesProposal,
};

use chrono::prelude::*;

use prost::Message;

use osmosis_std::types::cosmos::bank::v1beta1::SendAuthorization;
use osmosis_std::types::cosmos::base::v1beta1::Coin as BaseCoin;

const TWO_WEEKS_SECS: i64 = 14 * 24 * 60 * 60;

pub struct Authz<'a, R: Runner<'a>> {
    runner: &'a R,
}

impl<'a, R: Runner<'a>> Module<'a, R> for Authz<'a, R> {
    fn new(runner: &'a R) -> Self {
        Self { runner }
    }
}

impl<'a, R> Authz<'a, R>
where
    R: Runner<'a>,
{
    fn_execute! {
        pub exec: MsgExec["/cosmos.authz.v1beta1.MsgExec"] => MsgExecResponse
    }

    fn_execute! {
        pub grant: MsgGrant["/cosmos.authz.v1beta1.MsgGrant"] => MsgGrantResponse
    }

    fn_query! {
        pub query_grantee_grants ["/cosmos.authz.v1beta1.Query/GranteeGrants"]: QueryGranteeGrantsRequest => QueryGranteeGrantsResponse
    }

    fn_query! {
        pub query_granter_grants ["/cosmos.authz.v1beta1.Query/GranterGrants"]: QueryGranterGrantsRequest => QueryGranterGrantsResponse
    }

    fn_query! {
        pub query_grants ["/cosmos.authz.v1beta1.Query/Grants"]: QueryGrantsRequest => QueryGrantsResponse
    }
}

pub struct Setup {
    pub app: OsmosisTestApp,
    pub alice: SigningAccount,
    pub bob: SigningAccount,
}

impl Setup {
    pub fn new() -> Self {
        let app = OsmosisTestApp::new();

        let alice = app
            .init_account(&[
                Coin::new(1_000_000_000_000, "uatom"),
                Coin::new(1_000_000_000_000, "uosmo"),
            ])
            .unwrap();
        let bob = app
            .init_account(&[
                Coin::new(1_000_000_000_000, "uatom"),
                Coin::new(1_000_000_000_000, "uosmo"),
            ])
            .unwrap();
        Self { app, alice, bob }
    }
}

#[test]
fn query_grants_no_grants() {
    let Setup { app, alice, bob } = Setup::new();

    let authz = Authz::new(&app);

    let response = authz
        .query_grants(&QueryGrantsRequest {
            granter: bob.address(),
            grantee: alice.address(),
            msg_type_url: "/cosmos.bank.v1beta1.MsgSend".to_string(),
            pagination: None,
        })
        .unwrap_err();

    assert_eq!(
        response,
        QueryError {
            msg: "rpc error: code = NotFound desc = no authorization found for /cosmos.bank.v1beta1.MsgSend type"
                .to_string()
        }
    );
}

#[test]
fn query_grantee_grants_no_grants() {
    let Setup { app, alice, bob: _ } = Setup::new();

    let authz = Authz::new(&app);

    let response = authz
        .query_grantee_grants(&QueryGranteeGrantsRequest {
            grantee: alice.address(),
            pagination: None,
        })
        .unwrap();
    assert_eq!(response.grants, vec![]);
}

#[test]
fn query_grantee_grants_with_grant() {
    let Setup { app, alice, bob } = Setup::new();

    let authz = Authz::new(&app);

    let mut buf = vec![];
    SendAuthorization::encode(
        &SendAuthorization {
            spend_limit: vec![BaseCoin {
                amount: 10u128.to_string(),
                denom: "usdc".to_string(),
            }],
        },
        &mut buf,
    )
    .unwrap();

    let now = Utc::now();
    let ts: i64 = now.timestamp() + TWO_WEEKS_SECS;

    let expiration = OsmosisTimestamp {
        seconds: ts,
        nanos: 0_i32,
    };

    authz
        .grant(
            MsgGrant {
                granter: alice.address(),
                grantee: bob.address(),
                grant: Some(Grant {
                    authorization: Some(Any {
                        type_url: "/cosmos.bank.v1beta1.SendAuthorization".to_string(),
                        value: buf.clone(),
                    }),
                    expiration: Some(expiration.clone()),
                }),
            },
            &alice,
        )
        .unwrap();

    let response = authz
        .query_grantee_grants(&QueryGranteeGrantsRequest {
            grantee: bob.address(),
            pagination: None,
        })
        .unwrap();

    assert_eq!(
        response.grants,
        vec![GrantAuthorization {
            granter: alice.address(),
            grantee: bob.address(),
            authorization: Some(Any {
                type_url: "/cosmos.bank.v1beta1.SendAuthorization".to_string(),
                value: buf.clone(),
            }),
            expiration: Some(expiration),
        }]
    );
}

#[test]
fn query_granter_grants_no_grants() {
    let Setup { app, alice, bob: _ } = Setup::new();

    let authz = Authz::new(&app);

    let response = authz
        .query_granter_grants(&QueryGranterGrantsRequest {
            granter: alice.address(),
            pagination: None,
        })
        .unwrap();
    assert_eq!(response.grants, vec![]);
}

#[test]
fn pool_staking_grant() {
    let Setup { app, alice, bob: _ } = Setup::new();
    let gamm = Gamm::new(&app);
    let bank = Bank::new(&app);
    let authz = Authz::new(&app);

    let pool_liquidity = vec![Coin::new(1_000, "uatom"), Coin::new(1_000, "uosmo")];
    let pool_id = gamm
        .create_basic_pool(&pool_liquidity, &alice)
        .unwrap()
        .data
        .pool_id;

    let shares = app
        .query::<QueryTotalSharesRequest, QueryAllBalancesResponse>(
            "/osmosis.gamm.v1beta1.Query/TotalShares",
            &QueryTotalSharesRequest { pool_id },
        )
        .unwrap()
        .balances;

    //println!("{:#?}", shares);

    app.execute::<_, MsgLockTokensResponse>(
        MsgLockTokens {
            owner: alice.address(),
            duration: Some(osmosis_std::shim::Duration {
                seconds: 86400,
                nanos: 0,
            }),
            coins: shares,
        },
        MsgLockTokens::TYPE_URL,
        &alice,
    )
    .unwrap();

    // let balance_before = bank
    //     .query_balance(&QueryBalanceRequest {
    //         address: alice.address(),
    //         denom: "uosmo".to_string(),
    //     })
    //     .unwrap()
    //     .balance;

    // assert_eq!(
    //     balance_before.clone().unwrap().amount,
    //     "997687436500".to_string()
    // );

    // let guages = app
    //     .query::<GaugesRequest, GaugesResponse>(
    //         "/osmosis.incentives.Query/Gauges",
    //         &GaugesRequest { pagination: None },
    //     )
    //     .unwrap();

    // println!("{:#?}", guages);

    // let gauge_id = app
    //     .query::<QueryGaugeIdsRequest, QueryGaugeIdsResponse>(
    //         "/osmosis.poolincentives.v1beta1.Query/GaugeIds",
    //         &QueryGaugeIdsRequest { pool_id },
    //     )
    //     .unwrap();

    // println!("{:#?}", gauge_id);

    // Here I want to apply OSMO rewards to the pool and guage 1
    app.execute::<UpdatePoolIncentivesProposal, _>(
        UpdatePoolIncentivesProposal {
            title: "Foo".to_string(),
            description: "Bar".to_string(),
            records: vec![DistrRecord {
                gauge_id: 1,
                weight: 100.to_string(),
            }],
        },
        UpdatePoolIncentivesProposal::TYPE_URL,
        &alice,
    )
    .unwrap();

    // println!("{:#?}", update_gauges);

    app.increase_time(172800); // Forward two days
                               //
    let balance_after = bank
        .query_balance(&QueryBalanceRequest {
            address: alice.address(),
            denom: "uosmo".to_string(),
        })
        .unwrap()
        .balance;

    println!("{:#?}", balance_after);

    let response = authz
        .query_granter_grants(&QueryGranterGrantsRequest {
            granter: alice.address(),
            pagination: None,
        })
        .unwrap();
    assert_eq!(response.grants, vec![]);
}
