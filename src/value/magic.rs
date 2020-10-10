//! (De)serializable values that "magically" use information from the extracing
//! [`Figment`](crate::Figment).

use std::ops::Deref;
use std::path::{PathBuf, Path};

use serde::{Deserialize, Serialize, de};

use crate::{Error, value::{ConfiguredValueDe, MapDe, Id}};

/// Marker trait for "magic" values. Primarily for use with [`Either`].
pub trait Magic: for<'de> Deserialize<'de> {
    /// The name of the deserialization pseudo-strucure.
    #[doc(hidden)] const NAME: &'static str;

    /// The fields of the pseudo-structure. The last one should be the value.
    #[doc(hidden)] const FIELDS: &'static [&'static str];

    #[doc(hidden)] fn deserialize_from<'de: 'c, 'c, V: de::Visitor<'de>>(
        de: ConfiguredValueDe<'c>,
        visitor: V
    ) -> Result<V::Value, Error>;
}

/// A [`PathBuf`] that knows the path of the file it was configured in, if any.
///
/// Paths in configuration files are often desired to be relative to the
/// configuration file itself. For example, a path of `a/b.html` configured in a
/// file `/var/config.toml` might be desired to resolve as `/var/a/b.html`. This
/// type makes this possible by simply delcaring the configuration value's type
/// as [`RelativePathBuf`].
///
/// # Example
///
/// ```rust
/// use std::path::Path;
///
/// use serde::Deserialize;
/// use figment::{Figment, value::magic::RelativePathBuf, Jail};
/// use figment::providers::{Env, Format, Toml};
///
/// #[derive(Debug, PartialEq, Deserialize)]
/// struct Config {
///     path: RelativePathBuf,
/// }
///
/// Jail::expect_with(|jail| {
///     // When a path is declared in a file and deserialized as
///     // `RelativePathBuf`, `relative()` will be relative to the file.
///     jail.create_file("Config.toml", r#"path = "a/b/c.html""#)?;
///     let c: Config = Figment::from(Toml::file("Config.toml")).extract()?;
///     assert_eq!(c.path.original(), Path::new("a/b/c.html"));
///     assert_eq!(c.path.relative(), jail.directory().join("a/b/c.html"));
///
///     // If a path is declared elsewhere, the "relative" path is the original.
///     jail.set_env("PATH", "a/b/c.html");
///     let c: Config = Figment::from(Toml::file("Config.toml"))
///         .merge(Env::raw().only(&["PATH"]))
///         .extract()?;
///
///     assert_eq!(c.path.original(), Path::new("a/b/c.html"));
///     assert_eq!(c.path.relative(), Path::new("a/b/c.html"));
///
///     // Absolute paths remain unchanged.
///     jail.create_file("Config.toml", r#"path = "/var/c.html""#);
///     let c: Config = Figment::from(Toml::file("Config.toml")).extract()?;
///     assert_eq!(c.path.original(), Path::new("/var/c.html"));
///     assert_eq!(c.path.relative(), Path::new("/var/c.html"));
///
///     Ok(())
/// });
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename = "___figment_relative_path_buf")]
pub struct RelativePathBuf {
    #[serde(rename = "___figment_relative_metadata_path")]
    metadata_path: Option<PathBuf>,
    #[serde(rename = "___figment_relative_path")]
    path: PathBuf,
}

impl PartialEq for RelativePathBuf {
    fn eq(&self, other: &Self) -> bool {
        self.relative() == other.relative()
    }
}

impl<P: AsRef<Path>> From<P> for RelativePathBuf {
    fn from(path: P) -> RelativePathBuf {
        Self { metadata_path: None, path: path.as_ref().into() }
    }
}

impl Magic for RelativePathBuf {
    const NAME: &'static str = "___figment_relative_path_buf";

    const FIELDS: &'static [&'static str] = &[
        "___figment_relative_metadata_path",
        "___figment_relative_path"
    ];

    fn deserialize_from<'de: 'c, 'c, V: de::Visitor<'de>>(
        de: ConfiguredValueDe<'c>,
        visitor: V
    ) -> Result<V::Value, Error> {
        let config = de.config;
        let metadata_path = de.value.metadata_id()
            .and_then(|id| config.get_metadata(id))
            .and_then(|metadata| metadata.source.as_ref()
                .and_then(|s| s.file_path())
                .map(|path| path.display().to_string()));

        let mut map = crate::value::Map::new();
        if let Some(path) = metadata_path {
            map.insert(Self::FIELDS[0].into(), path.into());
        }

        map.insert(Self::FIELDS[1].into(), de.value.clone());
        visitor.visit_map(MapDe::new(&map, |v| ConfiguredValueDe::from(config, v)))
    }
}

impl RelativePathBuf {
    /// Returns the path as it was declared, without modification.
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::path::Path;
    ///
    /// use figment::{Figment, value::magic::RelativePathBuf, Jail};
    /// use figment::providers::{Format, Toml};
    ///
    /// #[derive(Debug, PartialEq, serde::Deserialize)]
    /// struct Config {
    ///     path: RelativePathBuf,
    /// }
    ///
    /// Jail::expect_with(|jail| {
    ///     jail.create_file("Config.toml", r#"path = "hello.html""#)?;
    ///     let c: Config = Figment::from(Toml::file("Config.toml")).extract()?;
    ///     assert_eq!(c.path.original(), Path::new("hello.html"));
    ///
    ///     Ok(())
    /// });
    /// ```
    pub fn original(&self) -> &Path {
        &self.path
    }

    /// Returns this path relative to the file it was delcared in, if any.
    /// Returns the original if this path was not declared in a file or if the
    /// path has a root.
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::path::Path;
    ///
    /// use figment::{Figment, value::magic::RelativePathBuf, Jail};
    /// use figment::providers::{Env, Format, Toml};
    ///
    /// #[derive(Debug, PartialEq, serde::Deserialize)]
    /// struct Config {
    ///     path: RelativePathBuf,
    /// }
    ///
    /// Jail::expect_with(|jail| {
    ///     jail.create_file("Config.toml", r#"path = "hello.html""#)?;
    ///     let c: Config = Figment::from(Toml::file("Config.toml")).extract()?;
    ///     assert_eq!(c.path.relative(), jail.directory().join("hello.html"));
    ///
    ///     jail.set_env("PATH", r#"hello.html"#);
    ///     let c: Config = Figment::from(Env::raw().only(&["PATH"])).extract()?;
    ///     assert_eq!(c.path.relative(), Path::new("hello.html"));
    ///
    ///     Ok(())
    /// });
    /// ```
    pub fn relative(&self) -> PathBuf {
        if self.original().has_root() {
            return self.original().into();
        }

        self.metadata_path()
            .and_then(|root| match root.is_dir() {
                true => Some(root),
                false => root.parent(),
            })
            .map(|root| root.join(self.original()))
            .unwrap_or_else(|| self.original().into())
    }

    /// Returns the path to the file this path was declared in, if any.
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::path::Path;
    ///
    /// use figment::{Figment, value::magic::RelativePathBuf, Jail};
    /// use figment::providers::{Env, Format, Toml};
    ///
    /// #[derive(Debug, PartialEq, serde::Deserialize)]
    /// struct Config {
    ///     path: RelativePathBuf,
    /// }
    ///
    /// Jail::expect_with(|jail| {
    ///     jail.create_file("Config.toml", r#"path = "hello.html""#)?;
    ///     let c: Config = Figment::from(Toml::file("Config.toml")).extract()?;
    ///     assert_eq!(c.path.metadata_path().unwrap(), jail.directory().join("Config.toml"));
    ///
    ///     jail.set_env("PATH", r#"hello.html"#);
    ///     let c: Config = Figment::from(Env::raw().only(&["PATH"])).extract()?;
    ///     assert_eq!(c.path.metadata_path(), None);
    ///
    ///     Ok(())
    /// });
    /// ```
    pub fn metadata_path(&self) -> Option<&Path> {
        self.metadata_path.as_ref().map(|p| p.as_ref())
    }
}

// #[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
// #[serde(rename = "___figment_selected_profile")]
// pub struct SelectedProfile {
//     profile: Profile,
// }
//
// /// TODO: This doesn't work when it's in a map and the config doesn't
// contain a value for the corresponding field; we never get to call
// `deserialize` on the field's value. We can't fabricate this from no value. We
// either need to fake the field name, somehow, or just not have this.
// impl Magic for SelectedProfile {
//     const NAME: &'static str = "___figment_selected_profile";
//     const FIELDS: &'static [&'static str] = &["profile"];
//
//     fn deserialize_from<'de: 'c, 'c, V: de::Visitor<'de>>(
//         de: ConfiguredValueDe<'c>,
//         visitor: V
//     ) -> Result<V::Value, Error>{
//         let mut map = crate::value::Map::new();
//         map.insert(Self::FIELDS[0].into(), de.config.profile().to_string().into());
//         visitor.visit_map(MapDe::new(&map, |v| ConfiguredValueDe::from(de.config, v)))
//     }
// }
//
// impl Deref for SelectedProfile {
//     type Target = Profile;
//
//     fn deref(&self) -> &Self::Target {
//         &self.profile
//     }
// }

/// (De)serializes as either a magic value `A` or any other deserializable value
/// `B`.
///
/// An `Either<A, B>` deserializes as either an `A` or `B`, whichever succeeds
/// first.
///
/// The usual `Either` implementation or an "untagged" enum does not allow its
/// internal values to provide hints to the deserializer. These hints are
/// required for magic values to work. By contrast, this `Either` _does_ provide
/// the appropriate hints.
///
/// # Example
///
/// ```
/// use serde::{Serialize, Deserialize};
/// use figment::{Figment, value::magic::{Either, RelativePathBuf, Tagged}};
///
/// #[derive(Debug, PartialEq, Deserialize)]
/// struct Config {
///     int_or_str: Either<Tagged<usize>, String>,
///     path_or_bytes: Either<RelativePathBuf, Vec<u8>>,
/// }
///
/// fn figment<A: Serialize, B: Serialize>(a: A, b: B) -> Figment {
///     Figment::from(("int_or_str", a)).merge(("path_or_bytes", b))
/// }
///
/// let config: Config = figment(10, "/a/b").extract().unwrap();
/// assert_eq!(config.int_or_str, Either::Left(10.into()));
/// assert_eq!(config.path_or_bytes, Either::Left("/a/b".into()));
///
/// let config: Config = figment("hi", "c/d").extract().unwrap();
/// assert_eq!(config.int_or_str, Either::Right("hi".into()));
/// assert_eq!(config.path_or_bytes, Either::Left("c/d".into()));
///
/// let config: Config = figment(123, &[1, 2, 3]).extract().unwrap();
/// assert_eq!(config.int_or_str, Either::Left(123.into()));
/// assert_eq!(config.path_or_bytes, Either::Right(vec![1, 2, 3].into()));
///
/// let config: Config = figment("boo!", &[4, 5, 6]).extract().unwrap();
/// assert_eq!(config.int_or_str, Either::Right("boo!".into()));
/// assert_eq!(config.path_or_bytes, Either::Right(vec![4, 5, 6].into()));
/// ```
#[derive(Serialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Either<A, B> {
    /// The "left" variant.
    Left(A),
    /// The "right" variant.
    Right(B),
}

impl<'de: 'b, 'b, A, B> Deserialize<'de> for Either<A, B>
    where A: Magic, B: Deserialize<'b>
{
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
        where D: de::Deserializer<'de>
    {
        use crate::value::ValueVisitor;

        // FIXME: propogate the error properly
        let value = de.deserialize_struct(A::NAME, A::FIELDS, ValueVisitor)?;
        // println!("initial value: {:?}", value);
        match A::deserialize(&value) {
            Ok(value) => Ok(Either::Left(value)),
            Err(a_err) => {
                let value = value.as_dict()
                    .and_then(|d| d.get(A::FIELDS[A::FIELDS.len() - 1]))
                    .unwrap_or(&value);

                match B::deserialize(value) {
                    Ok(value) => Ok(Either::Right(value)),
                    Err(b_err) => Err(de::Error::custom(format!("{}; {}", a_err, b_err)))
                }
            }
        }

        // use crate::error::Kind::*;
        // result.map_err(|e| match e.kind {
        //     InvalidType(Actual, String) => de::Error::invalid_type()
        //     InvalidValue(Actual, String),
        //     UnknownField(String, &'static [&'static str]),
        //     MissingField(Cow<'static, str>),
        //     DuplicateField(&'static str),
        //     InvalidLength(usize, String),
        //     UnknownVariant(String, &'static [&'static str]),
        //     kind => de::Error::custom(kind.to_string()),
        // })
    }
}

/// A wrapper around any value of type `T` and the metadata [`Id`] of the
/// provider that sourced the value.
///
/// ```rust
/// use figment::{Figment, value::magic::Tagged, Jail};
/// use figment::providers::{Format, Toml};
///
/// #[derive(Debug, PartialEq, serde::Deserialize)]
/// struct Config {
///     number: Tagged<usize>,
/// }
///
/// Jail::expect_with(|jail| {
///     jail.create_file("Config.toml", r#"number = 10"#)?;
///     let figment = Figment::from(Toml::file("Config.toml"));
///     let c: Config = figment.extract()?;
///     assert_eq!(*c.number, 10);
///
///     let metadata = c.number.metadata_id()
///         .and_then(|id| figment.get_metadata(id))
///         .expect("number has metadata id, figment has metadata");
///
///     assert_eq!(metadata.name, "TOML file");
///     Ok(())
/// });
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename = "___figment_tagged_item")]
pub struct Tagged<T> {
    #[serde(rename = "___figment_tagged_metadata_id")]
    metadata_id: Option<Id>,
    #[serde(rename = "___figment_tagged_value")]
    value: T,
}

impl<T: PartialEq> PartialEq for Tagged<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T: for<'de> Deserialize<'de>> Magic for Tagged<T> {
    const NAME: &'static str = "___figment_tagged_item";
    const FIELDS: &'static [&'static str] = &[
        "___figment_tagged_metadata_id" , "___figment_tagged_value"
    ];

    fn deserialize_from<'de: 'c, 'c, V: de::Visitor<'de>>(
        de: ConfiguredValueDe<'c>,
        visitor: V
    ) -> Result<V::Value, Error>{
        println!("from: tagged");
        let mut map = crate::value::Map::new();
        if let Some(id) = de.value.metadata_id() {
            map.insert(Self::FIELDS[0].into(), id.0.into());
        }

        let config = de.config;
        map.insert(Self::FIELDS[1].into(), de.value.clone());
        visitor.visit_map(MapDe::new(&map, |v| ConfiguredValueDe::from(config, v)))
    }
}

impl<T> Tagged<T> {
    /// Returns the ID of the metadata of the provider that sourced this value
    /// if it is known. As long `self` was extracted from a [`Figment`], the
    /// returned value is expected to be `Some`.
    ///
    /// The metadata can be retrieved by calling [`Figment::get_metadata()`] on
    /// the [`Figment`] `self` was extracted from.
    ///
    /// [`Figment`]: crate::Figment
    /// [`Figment::get_metadata()`]: crate::Figment::get_metadata()
    ///
    /// # Example
    ///
    /// ```rust
    /// use figment::{Figment, value::magic::Tagged};
    ///
    /// let figment = Figment::from(("key", "value"));
    /// let tagged = figment.extract_inner::<Tagged<String>>("key").unwrap();
    ///
    /// assert!(tagged.metadata_id().is_some());
    /// assert!(figment.get_metadata(tagged.metadata_id().unwrap()).is_some());
    /// ```
    pub fn metadata_id(&self) -> Option<Id> {
        self.metadata_id
    }

    /// Consumes `self` and returns the inner value.
    ///
    /// # Example
    ///
    /// ```rust
    /// use figment::{Figment, value::magic::Tagged};
    ///
    /// let tagged = Figment::from(("key", "value"))
    ///     .extract_inner::<Tagged<String>>("key")
    ///     .unwrap();
    ///
    /// let value = tagged.into_inner();
    /// assert_eq!(value, "value");
    /// ```
    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T> Deref for Tagged<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> From<T> for Tagged<T> {
    fn from(value: T) -> Self {
        Tagged { metadata_id: None, value, }
    }
}

#[cfg(test)]
mod tests {
    use crate::Figment;

    #[test]
    fn test_relative_path_buf() {
        use super::RelativePathBuf;
        use crate::providers::{Format, Toml};
        use std::path::Path;

        crate::Jail::expect_with(|jail| {
            jail.set_env("foo", "bar");
            jail.create_file("Config.toml", r###"
                [debug]
                file_path = "hello.js"
                another = "whoops/hi/there"
                absolute = "/tmp/foo"
            "###)?;

            let path: RelativePathBuf = Figment::new()
                .merge(Toml::file("Config.toml").nested())
                .select("debug")
                .extract_inner("file_path")?;

            assert_eq!(path.original(), Path::new("hello.js"));
            assert_eq!(path.metadata_path().unwrap(), jail.directory().join("Config.toml"));
            assert_eq!(path.relative(), jail.directory().join("hello.js"));

            let path: RelativePathBuf = Figment::new()
                .merge(Toml::file("Config.toml").nested())
                .select("debug")
                .extract_inner("another")?;

            assert_eq!(path.original(), Path::new("whoops/hi/there"));
            assert_eq!(path.metadata_path().unwrap(), jail.directory().join("Config.toml"));
            assert_eq!(path.relative(), jail.directory().join("whoops/hi/there"));

            let path: RelativePathBuf = Figment::new()
                .merge(Toml::file("Config.toml").nested())
                .select("debug")
                .extract_inner("absolute")?;

            assert_eq!(path.original(), Path::new("/tmp/foo"));
            assert_eq!(path.metadata_path().unwrap(), jail.directory().join("Config.toml"));
            assert_eq!(path.relative(), Path::new("/tmp/foo"));

            jail.create_file("Config.toml", r###"
                [debug.inner.container]
                inside = "inside_path/a.html"
            "###)?;

            #[derive(serde::Deserialize)]
            struct Testing { inner: Container, }

            #[derive(serde::Deserialize)]
            struct Container { container: Inside, }

            #[derive(serde::Deserialize)]
            struct Inside { inside: RelativePathBuf, }

            let testing: Testing = Figment::new()
                .merge(Toml::file("Config.toml").nested())
                .select("debug")
                .extract()?;

            let path = testing.inner.container.inside;
            assert_eq!(path.original(), Path::new("inside_path/a.html"));
            assert_eq!(path.metadata_path().unwrap(), jail.directory().join("Config.toml"));
            assert_eq!(path.relative(), jail.directory().join("inside_path/a.html"));

            Ok(())
        })
    }

    // #[test]
    // fn test_selected_profile() {
    //     use super::SelectedProfile;
    //
    //     let profile: SelectedProfile = Figment::new().select("foo").extract().unwrap();
    //     assert_eq!(profile.as_str(), "foo");
    //
    //     let profile: SelectedProfile = Figment::new().select("bar").extract().unwrap();
    //     assert_eq!(profile.as_str(), "bar");
    //
    //     #[derive(serde::Deserialize)]
    //     struct Testing {
    //         #[serde(alias = "other")]
    //         profile: SelectedProfile,
    //         value: usize
    //     }
    //
    //     let testing: Testing = Figment::from(("value", 123))
    //         .merge(("other", "hi"))
    //         .select("with-value").extract().unwrap();
    //
    //     assert_eq!(testing.profile.as_str(), "with-value");
    //     assert_eq!(testing.value, 123);
    // }

    #[test]
    fn test_tagged() {
        use super::Tagged;

        let val = Figment::from(("foo", "hello"))
            .extract_inner::<Tagged<String>>("foo")
            .expect("extraction");

        let first_tag = val.metadata_id().unwrap();
        assert_eq!(val.value, "hello");

        let val = Figment::from(("bar", "hi"))
            .extract_inner::<Tagged<String>>("bar")
            .expect("extraction");

        let second_tag = val.metadata_id().unwrap();
        assert_eq!(val.value, "hi");
        assert!(second_tag != first_tag);

        #[derive(serde::Deserialize)]
        struct TwoVals {
            foo: Tagged<String>,
            bar: Tagged<u16>,
        }

        let two = Figment::new()
            .merge(("foo", "hey"))
            .merge(("bar", 10))
            .extract::<TwoVals>()
            .expect("extraction");

        let tag3 = two.foo.metadata_id().unwrap();
        assert_eq!(two.foo.value, "hey");
        assert!(tag3 != second_tag);

        let tag4 = two.bar.metadata_id().unwrap();
        assert_eq!(two.bar.value, 10);
        assert!(tag4 != tag3);

        let val = Figment::new()
            .merge(("foo", "hey"))
            .merge(("bar", 10))
            .extract::<Tagged<TwoVals>>()
            .expect("extraction");

        assert_eq!(val.metadata_id(), None);

        let tag5 = val.value.foo.metadata_id().unwrap();
        assert_eq!(val.value.foo.value, "hey");
        assert!(tag4 != tag5);

        let tag6 = val.value.bar.metadata_id().unwrap();
        assert_eq!(val.value.bar.value, 10);
        assert!(tag6 != tag5)
    }
}
