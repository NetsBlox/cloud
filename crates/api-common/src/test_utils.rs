use crate::{ServiceHostId, ServiceName, SettingName};

pub trait TestFrom<T> {
    fn __from(value: T) -> Self; 
}

macro_rules! impl_test_from{
    ($type:ty) => {
        impl TestFrom<&str> for $type {
            fn __from(value:&str) -> Self {
                Self(value.to_string())
            }
        }
    };
}

impl_test_from!(ServiceHostId);
impl_test_from!(ServiceName);
impl_test_from!(SettingName);


