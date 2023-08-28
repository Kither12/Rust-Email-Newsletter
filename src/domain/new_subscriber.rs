use crate::domain::SubscriberEmail;
use crate::domain::SubscriberName;
use crate::routes::FormData;

pub struct NewSubscriber {
    pub email: SubscriberEmail,
    pub name: SubscriberName,
}
impl TryFrom<FormData> for NewSubscriber {
    type Error = String;
    fn try_from(form_data: FormData) -> Result<Self, Self::Error> {
        let subscriber_name = SubscriberName::parse(form_data.name.clone())?;
        let subscriber_email = SubscriberEmail::parse(form_data.email.clone())?;
        Ok(Self {
            email: subscriber_email,
            name: subscriber_name,
        })
    }
}
