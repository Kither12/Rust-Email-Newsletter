use lettre::Address;

use crate::domain::SubscriberName;
use crate::routes::FormData;

pub struct Subscriber {
    pub email: Address,
    pub name: SubscriberName,
}
impl TryFrom<FormData> for Subscriber {
    type Error = String;
    fn try_from(form_data: FormData) -> Result<Self, Self::Error> {
        let subscriber_name = SubscriberName::parse(form_data.name.clone())?;
        form_data.email.parse::<Address>().map(|email| Subscriber{email, name: subscriber_name}).map_err(|e| e.to_string())
    }
}
