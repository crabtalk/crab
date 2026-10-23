//! Requests to Neon's endpoint and the reading of its answers, against
//! arranged responses.

use berm_lang::{
    serde_json::{self, Value, json},
    test,
};
use crabkit_neon::{Neon, Statement};

const CONNECTION: &str = "postgresql://app:{secret:NEON_PASSWORD}@ep-cool-river-123456.us-east-2.aws.neon.tech/app?sslmode=require";
const ENDPOINT: &str = "https://api.us-east-2.aws.neon.tech/sql";

fn neon() -> Neon {
    CONNECTION.parse().unwrap()
}

fn body(index: usize) -> Value {
    serde_json::from_slice(&test::fetched()[index].body).unwrap()
}

#[test]
fn endpoint_is_the_hosts_api_subdomain() {
    assert_eq!(neon().endpoint, ENDPOINT);
    let pooled: Neon =
        "postgres://app:{secret:NEON_PASSWORD}@ep-x-pooler.eu-central-1.aws.neon.tech:5432/app"
            .parse()
            .unwrap();
    assert_eq!(
        pooled.endpoint,
        "https://api.eu-central-1.aws.neon.tech/sql"
    );
}

#[test]
fn a_connection_string_needs_a_postgres_scheme_and_a_qualified_host() {
    assert!(
        "mysql://app@ep-x.aws.neon.tech/app"
            .parse::<Neon>()
            .is_err()
    );
    assert!("postgresql://app@localhost/app".parse::<Neon>().is_err());
}

#[test]
fn a_query_sends_its_statement_with_the_secret_unsubstituted() {
    test::respond(
        "POST",
        ENDPOINT,
        200,
        br#"{"fields":[],"rows":[],"command":"SELECT","rowCount":0,"rowAsArray":true}"#,
    );
    let rows = neon()
        .rows::<(String,)>(
            "SELECT title FROM items WHERE id = $1 AND tag = $2",
            &(42, Some("a")),
        )
        .unwrap();
    assert!(rows.is_empty());

    let fetched = &test::fetched()[0];
    assert_eq!(fetched.method, "POST");
    assert_eq!(fetched.url, ENDPOINT);
    assert_eq!(
        fetched.headers,
        [
            ("Neon-Connection-String".into(), CONNECTION.into()),
            ("Neon-Raw-Text-Output".into(), "true".into()),
            ("Neon-Array-Mode".into(), "true".into()),
            ("Content-Type".into(), "application/json".into()),
        ]
    );
    assert_eq!(
        body(0),
        json!({"query": "SELECT title FROM items WHERE id = $1 AND tag = $2", "params": [42, "a"]})
    );
}

#[test]
fn a_transaction_sends_its_statements_in_one_request() {
    test::respond(
        "POST",
        ENDPOINT,
        200,
        br#"{"results":[
            {"fields":[],"rows":[],"command":"INSERT","rowCount":1,"rowAsArray":true},
            {"fields":[{"name":"count","dataTypeID":20}],"rows":[["3"]],"command":"SELECT","rowCount":1,"rowAsArray":true}
        ]}"#,
    );
    let answers = neon()
        .transaction(&[
            Statement::new("INSERT INTO items (title) VALUES ($1)", &("first",)).unwrap(),
            Statement::new("SELECT COUNT(*) FROM items", &[(); 0]).unwrap(),
        ])
        .unwrap();

    assert_eq!(test::fetched().len(), 1);
    assert_eq!(
        body(0),
        json!({"queries": [
            {"query": "INSERT INTO items (title) VALUES ($1)", "params": ["first"]},
            {"query": "SELECT COUNT(*) FROM items", "params": []},
        ]})
    );
    assert_eq!(answers[0].command, "INSERT");
    assert_eq!(answers[0].row_count, Some(1));
    assert_eq!(answers[1].rows::<(u64,)>().unwrap(), [(3,)]);
}

#[test]
fn rows_decode_by_column_type() {
    test::respond(
        "POST",
        ENDPOINT,
        200,
        br#"{
            "fields":[
                {"name":"ok","dataTypeID":16},
                {"name":"small","dataTypeID":21},
                {"name":"id","dataTypeID":23},
                {"name":"big","dataTypeID":20},
                {"name":"ratio","dataTypeID":701},
                {"name":"title","dataTypeID":25},
                {"name":"meta","dataTypeID":3802},
                {"name":"note","dataTypeID":25}
            ],
            "rows":[
                ["t","-2","7","9007199254740993","0.5","hello","{\"a\":[1,2]}",null],
                ["f","0","8","-1","NaN","","null","x"]
            ],
            "command":"SELECT","rowCount":2,"rowAsArray":true
        }"#,
    );
    type Row = (bool, i16, i32, i64, Value, String, Value, Option<String>);
    let rows = neon().rows::<Row>("SELECT * FROM t", &[(); 0]).unwrap();
    assert_eq!(
        rows,
        [
            (
                true,
                -2,
                7,
                9_007_199_254_740_993,
                json!(0.5),
                "hello".into(),
                json!({"a": [1, 2]}),
                None
            ),
            (
                false,
                0,
                8,
                -1,
                json!("NaN"),
                String::new(),
                Value::Null,
                Some("x".into())
            ),
        ]
    );
}

#[test]
fn execute_answers_the_rows_affected() {
    test::respond(
        "POST",
        ENDPOINT,
        200,
        br#"{"fields":[],"rows":[],"command":"UPDATE","rowCount":4,"rowAsArray":true}"#,
    );
    let affected = neon()
        .execute("UPDATE items SET done = $1", &(true,))
        .unwrap();
    assert_eq!(affected, 4);
    assert_eq!(body(0)["params"], json!([true]));
}

#[test]
fn a_postgres_error_names_neon_the_status_and_the_code() {
    test::respond(
        "POST",
        ENDPOINT,
        400,
        br#"{"message":"relation \"missing\" does not exist","code":"42P01","detail":"","severity":"ERROR"}"#,
    );
    let error = neon()
        .rows::<(i32,)>("SELECT 1 FROM missing", &[(); 0])
        .unwrap_err();
    assert_eq!(
        error.message,
        r#"Neon answered 400: relation "missing" does not exist (42P01)"#
    );
}

#[test]
fn an_error_without_a_code_or_a_json_body_still_names_neon_and_the_status() {
    test::respond(
        "POST",
        ENDPOINT,
        500,
        br#"{"message":"Query cancelled, connection was terminated","code":""}"#,
    );
    let error = neon().execute("SELECT 1", &[(); 0]).unwrap_err();
    assert_eq!(
        error.message,
        "Neon answered 500: Query cancelled, connection was terminated"
    );

    test::respond("POST", ENDPOINT, 502, b"bad gateway");
    let error = neon().execute("SELECT 1", &[(); 0]).unwrap_err();
    assert_eq!(error.message, "Neon answered 502: bad gateway");
}

#[test]
fn parameters_must_be_an_array() {
    assert!(Statement::new("SELECT $1", &42).is_err());
}
