use rust_email_newsletter::configuration::*;
use rust_email_newsletter::startup::Application;
use rust_email_newsletter::telemetry::init_subscriber;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    init_subscriber("email_newsletter", "info", std::io::stdout);

    let configuration = get_configuration().expect("Failed to read configuration");
    let application = Application::build(&configuration).await?;
    application.run_until_stopped().await?;
    Ok(())
}
