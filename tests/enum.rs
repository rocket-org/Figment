use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct Test {
    service: Option<Foo>,
}

#[derive(PartialEq, Debug, Deserialize, Serialize)]
pub enum Foo {
    Mega,
    Supa,
}

#[test]
fn test_enum_de() {
    let figment = || {
        Figment::new()
            .merge(Serialized::defaults(Test::default()))
            .merge(Toml::file("Test.toml"))
            .merge(Env::prefixed("TEST_"))
    };

    figment::Jail::expect_with(|jail| {
        jail.create_file("Test.toml", "service = \"Mega\"")?;

        let test: Test = figment().extract()?;
        assert_eq!(test.service, Some(Foo::Mega));

        jail.set_env("TEST_SERVICE", "Supa");

        let test: Test = figment().extract()?;
        assert_eq!(test.service, Some(Foo::Supa));

        Ok(())
    })
}
