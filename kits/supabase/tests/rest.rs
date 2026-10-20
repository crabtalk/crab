//! PostgREST requests and answers, against arranged responses.

use berm_lang::{
    serde::Deserialize,
    serde_json::{self, json},
    test::{self, Fetched},
};
use crab_supabase::Supabase;

const TODOS: &str = "https://abcd.supabase.co/rest/v1/todos";

#[derive(Debug, PartialEq, Deserialize)]
#[serde(crate = "berm_lang::serde")]
struct Todo {
    id: u64,
    title: String,
    done: bool,
}

fn supabase() -> Supabase {
    test::forget();
    Supabase::new("https://abcd.supabase.co/")
}

fn headers(extra: &[(&str, &str)]) -> Vec<(String, String)> {
    [
        ("apikey", "{secret:SUPABASE_KEY}"),
        ("Authorization", "Bearer {secret:SUPABASE_KEY}"),
    ]
    .iter()
    .chain(extra)
    .map(|(name, value)| (name.to_string(), value.to_string()))
    .collect()
}

fn only() -> Fetched {
    let fetched = test::fetched();
    assert_eq!(fetched.len(), 1, "{fetched:?}");
    fetched.into_iter().next().unwrap()
}

#[test]
fn get_sends_columns_filters_order_and_range() {
    let supabase = supabase();
    let url = format!(
        "{TODOS}?select=id%2Ctitle%2Cdone&done=eq.false&title=ilike.%2Amilk%2A&limit=10&offset=20&order=id.desc%2Ctitle.asc"
    );
    test::respond(
        "GET",
        &url,
        200,
        br#"[{"id":1,"title":"buy milk","done":false}]"#,
    );

    let todos: Vec<Todo> = supabase
        .from("todos")
        .select("id,title,done")
        .eq("done", "false")
        .ilike("title", "*milk*")
        .order("id", false)
        .limit(10)
        .offset(20)
        .order("title", true)
        .get()
        .unwrap();

    assert_eq!(
        todos,
        [Todo {
            id: 1,
            title: "buy milk".into(),
            done: false
        }]
    );
    assert_eq!(
        only(),
        Fetched {
            method: "GET".into(),
            url,
            headers: headers(&[]),
            body: Vec::new(),
        }
    );
}

#[test]
fn get_without_a_query_sends_the_bare_table() {
    let supabase = supabase();
    test::respond("GET", TODOS, 200, b"[]");

    let todos: Vec<Todo> = supabase.from("todos").get().unwrap();

    assert!(todos.is_empty());
    assert_eq!(only().url, TODOS);
}

#[test]
fn values_and_table_names_are_percent_encoded() {
    let supabase = supabase();
    let url = "https://abcd.supabase.co/rest/v1/my%20todos?title=eq.a%26b%3Dc%20d";
    test::respond("GET", url, 200, b"[]");

    let _: Vec<Todo> = supabase
        .from("my todos")
        .eq("title", "a&b=c d")
        .get()
        .unwrap();

    assert_eq!(only().url, url);
}

#[test]
fn insert_posts_rows_and_answers_them() {
    let supabase = supabase();
    let url = format!("{TODOS}?select=%2A");
    test::respond(
        "POST",
        &url,
        201,
        br#"[{"id":7,"title":"walk","done":false}]"#,
    );

    let rows = json!([{"title": "walk"}]);
    let inserted: Vec<Todo> = supabase
        .from("todos")
        .select("*")
        .eq("done", "true")
        .insert(&rows)
        .unwrap();

    assert_eq!(inserted[0].id, 7);
    assert_eq!(
        only(),
        Fetched {
            method: "POST".into(),
            url,
            headers: headers(&[
                ("Content-Type", "application/json"),
                ("Prefer", "return=representation"),
            ]),
            body: serde_json::to_vec(&rows).unwrap(),
        }
    );
}

#[test]
fn update_patches_the_filtered_rows() {
    let supabase = supabase();
    let url = format!("{TODOS}?id=eq.7");
    test::respond(
        "PATCH",
        &url,
        200,
        br#"[{"id":7,"title":"walk","done":true}]"#,
    );

    let values = json!({"done": true});
    let updated: Vec<Todo> = supabase
        .from("todos")
        .eq("id", "7")
        .update(&values)
        .unwrap();

    assert!(updated[0].done);
    assert_eq!(
        only(),
        Fetched {
            method: "PATCH".into(),
            url,
            headers: headers(&[
                ("Content-Type", "application/json"),
                ("Prefer", "return=representation"),
            ]),
            body: br#"{"done":true}"#.to_vec(),
        }
    );
}

#[test]
fn delete_answers_the_deleted_rows() {
    let supabase = supabase();
    let url = format!("{TODOS}?id=in.%281%2C2%29");
    test::respond(
        "DELETE",
        &url,
        200,
        br#"[{"id":1,"title":"a","done":true},{"id":2,"title":"b","done":true}]"#,
    );

    let deleted: Vec<Todo> = supabase
        .from("todos")
        .filter("id", "in", "(1,2)")
        .delete()
        .unwrap();

    assert_eq!(deleted.len(), 2);
    assert_eq!(
        only(),
        Fetched {
            method: "DELETE".into(),
            url,
            headers: headers(&[("Prefer", "return=representation")]),
            body: Vec::new(),
        }
    );
}

#[test]
fn rpc_posts_named_arguments() {
    let supabase = supabase();
    let url = "https://abcd.supabase.co/rest/v1/rpc/add_them";
    test::respond("POST", url, 200, b"5");

    let sum: i64 = supabase.rpc("add_them", &json!({"a": 2, "b": 3})).unwrap();

    assert_eq!(sum, 5);
    assert_eq!(
        only(),
        Fetched {
            method: "POST".into(),
            url: url.into(),
            headers: headers(&[("Content-Type", "application/json")]),
            body: br#"{"a":2,"b":3}"#.to_vec(),
        }
    );
}

#[test]
fn a_postgrest_error_names_supabase_the_status_message_and_code() {
    let supabase = supabase();
    test::respond(
        "GET",
        "https://abcd.supabase.co/rest/v1/nope",
        404,
        br#"{"code":"PGRST205","details":null,"hint":null,"message":"Could not find the table 'public.nope' in the schema cache"}"#,
    );

    let failure = supabase.from("nope").get::<Todo>().unwrap_err();

    assert_eq!(
        failure.message,
        "Supabase answered 404: Could not find the table 'public.nope' in the schema cache (PGRST205)"
    );
}

#[test]
fn an_unreadable_error_body_names_the_status() {
    let supabase = supabase();
    test::respond("GET", TODOS, 502, b"<html>bad gateway</html>");

    let failure = supabase.from("todos").get::<Todo>().unwrap_err();

    assert_eq!(failure.message, "Supabase answered 502");
}

#[test]
fn rows_of_another_shape_fail() {
    let supabase = supabase();
    test::respond("GET", TODOS, 200, br#"[{"id":"x"}]"#);

    let failure = supabase.from("todos").get::<Todo>().unwrap_err();

    assert_eq!(
        failure.message,
        "Supabase answered what this kit cannot read"
    );
}
