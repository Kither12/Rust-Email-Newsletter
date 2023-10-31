use crate::helpers::spawn_app;
use test_case::test_case;
#[tokio::test]
async fn check_form_data_valid() {
    let app = spawn_app().await;
    let body = "name=kither%20god&email=toantqm2509%40gmail.com";

    let response = app.post_subscriptions(body).await;
    assert_eq!(200, response.status().as_u16());

    let saved = sqlx::query!("SELECT email, name FROM subscriptions")
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved subscription");

    assert_eq!(saved.name, "kither god");
    assert_eq!(saved.email, "toantqm2509@gmail.com");
}

#[test_case("name=kither%20god"; "form_data_missing_email")]
#[test_case("email=kither_123%40gmail.com"; "form_data_missing_name")]
#[test_case(""; "missing_both_email_and_name")]
#[tokio::test]
async fn check_form_data_unvalid(body: &'static str) {
    let app = spawn_app().await;
    let response = app.post_subscriptions(body).await;
    assert_eq!(400, response.status().as_u16());
}

#[test_case("name=&email=kither_123%40gmail.com"; "name_is_empty")]
#[test_case("name=sadfsdf&email="; "email_is_empty")]
#[test_case("name=sadfsdf&email=dsrgerg"; "email_is_invalid")]
#[tokio::test]
async fn check_form_present_but_unvalid(body: &'static str) {
    let app = spawn_app().await;
    let response = app.post_subscriptions(body).await;
    assert_eq!(400, response.status().as_u16());
}

#[tokio::test]
async fn subscribe_fails_if_there_is_a_fatal_database_error() {
    let app = spawn_app().await;
    let body = "name=kither%20god&email=toantqm2509%40gmail.com";
    sqlx::query!("ALTER TABLE subscription_tokens DROP COLUMN subscription_token;",)
        .execute(&app.db_pool)
        .await
        .unwrap();
    let response = app.post_subscriptions(body.into()).await;
    assert_eq!(response.status().as_u16(), 500);
}
