//! Storage requests and answers, against arranged responses.

use berm_lang::test::{self, Fetched};
use crab_supabase::Supabase;

const OBJECT: &str = "https://abcd.supabase.co/storage/v1/object/avatars/users/ada%20l.png";

fn supabase() -> Supabase {
    test::forget();
    Supabase::new("https://abcd.supabase.co")
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
fn upload_posts_the_bytes_under_their_type() {
    let supabase = supabase();
    test::respond(
        "POST",
        OBJECT,
        200,
        br#"{"Key":"avatars/users/ada l.png","Id":"1"}"#,
    );

    supabase
        .storage("avatars")
        .upload("users/ada l.png", "image/png", b"\x89PNG", false)
        .unwrap();

    assert_eq!(
        only(),
        Fetched {
            method: "POST".into(),
            url: OBJECT.into(),
            headers: headers(&[("Content-Type", "image/png"), ("x-upsert", "false")]),
            body: b"\x89PNG".to_vec(),
        }
    );
}

#[test]
fn upsert_says_so() {
    let supabase = supabase();
    test::respond("POST", OBJECT, 200, b"{}");

    supabase
        .storage("avatars")
        .upload("users/ada l.png", "image/png", b"", true)
        .unwrap();

    assert!(only().headers.contains(&("x-upsert".into(), "true".into())));
}

#[test]
fn download_answers_the_bytes() {
    let supabase = supabase();
    test::respond("GET", OBJECT, 200, b"\x89PNG");

    let bytes = supabase
        .storage("avatars")
        .download("users/ada l.png")
        .unwrap();

    assert_eq!(bytes, b"\x89PNG");
    assert_eq!(
        only(),
        Fetched {
            method: "GET".into(),
            url: OBJECT.into(),
            headers: headers(&[]),
            body: Vec::new(),
        }
    );
}

#[test]
fn delete_deletes_the_one_object() {
    let supabase = supabase();
    test::respond(
        "DELETE",
        OBJECT,
        200,
        br#"{"message":"Successfully deleted"}"#,
    );

    supabase
        .storage("avatars")
        .delete("users/ada l.png")
        .unwrap();

    assert_eq!(
        only(),
        Fetched {
            method: "DELETE".into(),
            url: OBJECT.into(),
            headers: headers(&[]),
            body: Vec::new(),
        }
    );
}

#[test]
fn a_storage_error_names_supabase_the_status_message_and_error() {
    let supabase = supabase();
    test::respond(
        "GET",
        OBJECT,
        400,
        br#"{"statusCode":"404","error":"not_found","message":"Object not found"}"#,
    );

    let failure = supabase
        .storage("avatars")
        .download("users/ada l.png")
        .unwrap_err();

    assert_eq!(
        failure.message,
        "Supabase answered 400: Object not found (not_found)"
    );
}

#[test]
fn a_path_out_of_its_bucket_sends_nothing() {
    let bucket = supabase().storage("avatars");
    for path in ["../other/x", "users/./x", "users//x", "/x", "x/.."] {
        let failure = bucket.download(path).unwrap_err();
        assert!(
            failure.message.contains("is not an object path"),
            "{path}: {failure}"
        );
    }
    assert!(test::fetched().is_empty());
}
