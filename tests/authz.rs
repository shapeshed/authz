use osmosis_test_tube::{Account, OsmosisTestApp, Runner, RunnerError::QueryError};

use osmosis_std::types::cosmos::authz::v1beta1::{QueryGrantsRequest, QueryGrantsResponse, QueryGranterGrantsRequest, QueryGranterGrantsResponse, QueryGranteeGrantsResponse, QueryGranteeGrantsRequest};
use cosmwasm_std::Coin;


#[test]
fn query_grants() {
    let app = OsmosisTestApp::new();
    let accs = app
    .init_accounts(
        &[
            Coin::new(1_000_000_000_000, "uatom"),
            Coin::new(1_000_000_000_000, "uosmo"),
        ],
        2,
    )
    .unwrap();
    let alice = &accs[0];
    let bob = &accs[1];

    let res = app.query::<QueryGrantsRequest, QueryGrantsResponse>(
        "/cosmos.authz.v1beta1.Query/Grants",
        &QueryGrantsRequest {
            granter:  bob.address(),
            grantee: alice.address(),
            msg_type_url: "/cosmos.bank.v1beta1.MsgSend".to_string(),
            pagination: None,
        },
    );

    let err = res.unwrap_err();
    assert_eq!(
        err,
        QueryError {
            msg: "rpc error: code = NotFound desc = no authorization found for /cosmos.bank.v1beta1.MsgSend type"
                .to_string()
        }
    );
}

#[test]
fn query_granter_grants() {
    let app = OsmosisTestApp::new();
    let accs = app
    .init_accounts(
        &[
            Coin::new(1_000_000_000_000, "uatom"),
            Coin::new(1_000_000_000_000, "uosmo"),
        ],
        1,
    )
    .unwrap();
    let alice = &accs[0];

    let res = app.query::<QueryGranterGrantsRequest, QueryGranterGrantsResponse>(
        "/cosmos.authz.v1beta1.Query/GranterGrants",
        &QueryGranterGrantsRequest {
            granter:  alice.address(),
            pagination: None,
        },
    ).unwrap();

    let vec = Vec::new();
    assert_eq!(
        res.grants,
        vec,
    );
}

#[test]
fn query_grantee_grants() {
    let app = OsmosisTestApp::new();
    let accs = app
    .init_accounts(
        &[
            Coin::new(1_000_000_000_000, "uatom"),
            Coin::new(1_000_000_000_000, "uosmo"),
        ],
        1,
    )
    .unwrap();
    let alice = &accs[0];

    let res = app.query::<QueryGranteeGrantsRequest, QueryGranteeGrantsResponse>(
        "/cosmos.authz.v1beta1.Query/GranteeGrants",
        &QueryGranteeGrantsRequest {
            grantee:  alice.address(),
            pagination: None,
        },
    ).unwrap();

    let vec = Vec::new();
    assert_eq!(
        res.grants,
        vec,
    );
}
