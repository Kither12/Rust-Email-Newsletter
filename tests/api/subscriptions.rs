use crate::helpers::spawn_app;
use rust_email_newsletter::email_client::EmailClient;
use test_case::test_case;
#[tokio::test]
async fn check_form_data_valid() {
    let app = spawn_app().await;
    let body = "name=testName&email=testEmail%40gmail.com";

    let response = app.post_subscriptions(body).await;
    assert_eq!(200, response.status().as_u16());
    
    let saved = sqlx::query!("SELECT email, name FROM subscriptions")
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved subscription");

    assert_eq!(saved.email, "testEmail@gmail.com");
    assert_eq!(saved.name, "testName");

    //Check confirrmation mail are send
    let subscription_token = response.text().await.expect("Failed to get response");
    let confimation_link = EmailClient::get_confirmation_link("http://127.0.0.1", &subscription_token);
    assert!(app.check_confirmation_mail_exist(confimation_link));

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
    let body = "name=testName&email=testEmail%40gmail.com";
    sqlx::query!("ALTER TABLE subscription_tokens DROP COLUMN subscription_token;",)
        .execute(&app.db_pool)
        .await
        .unwrap();
    let response = app.post_subscriptions(body.into()).await;
    assert_eq!(response.status().as_u16(), 500);
}