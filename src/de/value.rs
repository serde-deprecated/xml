
use error::{Error, ErrorCode};
use serde::de;
use std::{mem, vec};
use std::collections::btree_map;
use value::{Content, Element};

pub struct Deserializer {
    value: Option<Element>,
}

impl Deserializer {
    /// Creates a new deserializer instance for deserializing the specified JSON value.
    pub fn new(value: Element) -> Deserializer {
        Deserializer { value: Some(value) }
    }
}
impl de::Deserializer for Deserializer {
    type Error = Error;

    de_forward_to_deserialize!{
        deserialize_bool,
        deserialize_f64, deserialize_f32,
        deserialize_u8, deserialize_u16, deserialize_u32, deserialize_u64, deserialize_usize,
        deserialize_i8, deserialize_i16, deserialize_i32, deserialize_i64, deserialize_isize,
        deserialize_char, deserialize_str, deserialize_string,
        deserialize_ignored_any,
        deserialize_bytes,
        deserialize_unit_struct, deserialize_unit,
        deserialize_seq, deserialize_seq_fixed_size,
        deserialize_newtype_struct, deserialize_struct_field,
        deserialize_tuple,
        deserialize_struct, deserialize_tuple_struct
    }

    fn deserialize_ignored_any<V>(&mut self, _visitor: V) -> Result<V::Value, Self::Error>
        where V: de::Visitor,
    {
        unimplemented!()
    }

    #[inline]
    fn deserialize<V>(&mut self, mut visitor: V) -> Result<V::Value, Error>
        where V: de::Visitor,
    {
        use self::MapDeserializerState::*;
        debug!("value::Deserializer::deserialize {:?}", self.value);
        let el = match self.value.take() {
            Some(value) => value,
            None => {
                return Err(de::Error::end_of_stream());
            },
        };

        match (el.attributes.is_empty(), el.members) {
            (true, Content::Text(s)) => visitor.visit_string(s),
            (true, Content::Nothing) => visitor.visit_unit(),
            (_, m) => visitor.visit_map(MapDeserializer {
                attributes: el.attributes
                              .into_iter()
                              .map(|(k, v)| (k, v.into_iter()))
                              .collect(),
                state: Inner,
                members: m,
            }),
        }
    }

    #[inline]
    fn deserialize_option<V>(&mut self, mut visitor: V) -> Result<V::Value, Error>
        where V: de::Visitor,
    {
        debug!("value::Deserializer::deserialize_option");
        if self.value.is_none() {
            return Err(de::Error::end_of_stream());
        };
        if self.value == Some(Element::new_empty()) {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    #[inline]
    fn deserialize_enum<V>(&mut self,
                           _name: &str,
                           _variants: &'static [&'static str],
                           mut visitor: V)
                           -> Result<V::Value, Error>
        where V: de::EnumVisitor,
    {
        debug!("value::Deserializer::deserialize_enum");
        visitor.visit(VariantVisitor(self.value.take()))
    }

    #[inline]
    fn deserialize_map<V>(&mut self, mut visitor: V) -> Result<V::Value, Error>
        where V: de::Visitor,
    {
        use self::MapDeserializerState::*;
        debug!("value::Deserializer::deserialize_map {:?}", self.value);
        let el = match self.value.take() {
            Some(value) => value,
            None => {
                return Err(de::Error::end_of_stream());
            },
        };
        visitor.visit_map(MapDeserializer {
            attributes: el.attributes
                          .into_iter()
                          .map(|(k, v)| (k, v.into_iter()))
                          .collect(),
            state: Inner,
            members: el.members,
        })
    }
}

struct VariantVisitor(Option<Element>);

impl de::VariantVisitor for VariantVisitor {
    type Error = Error;

    fn visit_variant<V>(&mut self) -> Result<V, Self::Error>
        where V: de::Deserialize,
    {
        debug!("VariantVisitor::visit_variant");
        if let Some(s) = self.0.as_mut().unwrap().attributes.remove("xsi:type") {
            de::Deserialize::deserialize(&mut StringDeserializer(s.into_iter().next()))
        } else {
            return Err(Error::SyntaxError(ErrorCode::Expected("attribute xsi:type"), 0, 0));
        }
    }

    /// `visit_unit` is called when deserializing a variant with no values.
    fn visit_unit(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    /// `visit_tuple` is called when deserializing a tuple-like variant.
    fn visit_tuple<V>(&mut self, _len: usize, _visitor: V) -> Result<V::Value, Self::Error>
        where V: de::Visitor,
    {
        unimplemented!()
    }

    fn visit_newtype<T>(&mut self) -> Result<T, Self::Error>
        where T: de::Deserialize,
    {
        debug!("newtype deserialization");
        // unwrap can never panic, since the Option is always Some
        let element = self.0.take().unwrap();
        de::Deserialize::deserialize(&mut Deserializer::new(element))
    }

    /// `visit_struct` is called when deserializing a struct-like variant.
    fn visit_struct<V>(&mut self, _fields: &'static [&'static str], mut visitor: V) -> Result<V::Value, Self::Error>
        where V: de::Visitor,
    {
        // unwrap can never panic, since the Option is always Some
        let element = self.0.take().unwrap();
        visitor.visit_map(MapDeserializer {
            attributes: element.attributes
                          .into_iter()
                          .map(|(k, v)| (k, v.into_iter()))
                          .collect(),
            state: MapDeserializerState::Inner,
            members: element.members,
        })
    }
}

struct SeqDeserializer<I: Iterator<Item = Element> + ExactSizeIterator>(I);

impl<I> de::Deserializer for SeqDeserializer<I>
    where I: Iterator<Item = Element>,
          I: ExactSizeIterator,
{
    type Error = Error;

    de_forward_to_deserialize!{
        deserialize_bool,
        deserialize_f64, deserialize_f32,
        deserialize_u8, deserialize_u16, deserialize_u32, deserialize_u64, deserialize_usize,
        deserialize_i8, deserialize_i16, deserialize_i32, deserialize_i64, deserialize_isize,
        deserialize_char, deserialize_str, deserialize_string,
        deserialize_ignored_any,
        deserialize_bytes,
        deserialize_unit_struct, deserialize_unit,
        deserialize_seq_fixed_size,
        deserialize_map, deserialize_newtype_struct, deserialize_struct_field,
        deserialize_tuple,
        deserialize_struct, deserialize_tuple_struct,
        deserialize_option
    }

    fn deserialize_ignored_any<V>(&mut self, _visitor: V) -> Result<V::Value, Self::Error>
        where V: de::Visitor,
    {
        unimplemented!()
    }

    #[inline]
    fn deserialize<V>(&mut self, mut visitor: V) -> Result<V::Value, Error>
        where V: de::Visitor,
    {
        debug!("seqdeserializer::deserialize");
        if let Some(el) = self.0.next() {
            debug!("el");
            de::Deserialize::deserialize(&mut Deserializer::new(el))
        } else {
            debug!("unit");
            visitor.visit_unit()
        }
    }

    #[inline]
    fn deserialize_enum<V>(&mut self,
                           _name: &str,
                           _variants: &'static [&'static str],
                           mut visitor: V)
                           -> Result<V::Value, Error>
        where V: de::EnumVisitor,
    {
        debug!("value::Deserializer::deserialize_enum");
        visitor.visit(VariantVisitor(self.0.next()))
    }

    #[inline]
    fn deserialize_seq<V>(&mut self, mut visitor: V) -> Result<V::Value, Error>
        where V: de::Visitor,
    {
        debug!("seqdeserializer::deserialize_seq");
        visitor.visit_seq(self)
    }
}

impl<I> de::SeqVisitor for SeqDeserializer<I>
    where I: Iterator<Item = Element>,
          I: ExactSizeIterator,
{
    type Error = Error;

    fn visit<T>(&mut self) -> Result<Option<T>, Error>
        where T: de::Deserialize,
    {
        debug!("SeqDeserializer::deserialize");
        match self.0.next() {
            Some(value) => {
                debug!("value: {:?}", value);
                de::Deserialize::deserialize(&mut Deserializer::new(value)).map(Some)
            },
            None => Ok(None),
        }
    }

    fn end(&mut self) -> Result<(), Error> {
        debug!("SeqDeserializer::end");
        if self.0.len() == 0 {
            Ok(())
        } else {
            // FIXME: should be trailing whitespace error?
            Err(de::Error::end_of_stream())
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.0.len(), Some(self.0.len()))
    }
}

struct StringDeserializer(Option<String>);

impl de::Deserializer for StringDeserializer {
    type Error = Error;

    de_forward_to_deserialize!{
        deserialize_bool,
        deserialize_f64, deserialize_f32,
        deserialize_u8, deserialize_u16, deserialize_u32, deserialize_u64, deserialize_usize,
        deserialize_i8, deserialize_i16, deserialize_i32, deserialize_i64, deserialize_isize,
        deserialize_char, deserialize_str, deserialize_string,
        deserialize_ignored_any,
        deserialize_bytes,
        deserialize_unit_struct, deserialize_unit,
        deserialize_seq, deserialize_seq_fixed_size,
        deserialize_map, deserialize_newtype_struct, deserialize_struct_field,
        deserialize_tuple,
        deserialize_enum,
        deserialize_struct, deserialize_tuple_struct,
        deserialize_option
    }

    fn deserialize_ignored_any<V>(&mut self, _visitor: V) -> Result<V::Value, Self::Error>
        where V: de::Visitor,
    {
        unimplemented!()
    }

    #[inline]
    fn deserialize<V>(&mut self, mut visitor: V) -> Result<V::Value, Error>
        where V: de::Visitor,
    {
        visitor.visit_string(self.0.take().unwrap())
    }
}

#[derive(PartialEq, Debug)]
enum MapDeserializerState {
    Inner,
    Done,
}

struct MapDeserializer {
    attributes: btree_map::BTreeMap<String, vec::IntoIter<String>>,
    members: Content,
    state: MapDeserializerState,
}

impl de::MapVisitor for MapDeserializer {
    type Error = Error;

    fn visit_key<T>(&mut self) -> Result<Option<T>, Error>
        where T: de::Deserialize,
    {
        Ok(None)
    }

    fn visit_value<T>(&mut self) -> Result<T, Error>
        where T: de::Deserialize,
    {
        unreachable!()
    }

    fn end(&mut self) -> Result<(), Error> {
        debug!("value::MapDeserializer::end");
        Ok(())
    }

    fn missing_field<V>(&mut self, field: &'static str) -> Result<V, Error>
        where V: de::Deserialize,
    {
        use self::MapDeserializerState::*;
        debug!("value::MapDeserializer::missing_field {:?} {}",
               self.state,
               field);

        // See if the type can deserialize from a unit.
        struct UnitDeserializer;

        impl de::Deserializer for UnitDeserializer {
            type Error = Error;

            de_forward_to_deserialize!{
                deserialize_bool,
                deserialize_f64, deserialize_f32,
                deserialize_u8, deserialize_u16, deserialize_u32, deserialize_u64, deserialize_usize,
                deserialize_i8, deserialize_i16, deserialize_i32, deserialize_i64, deserialize_isize,
                deserialize_char, deserialize_str, deserialize_string,
                deserialize_ignored_any,
                deserialize_bytes,
                deserialize_unit_struct, deserialize_unit,
                deserialize_seq, deserialize_seq_fixed_size,
                deserialize_map, deserialize_newtype_struct, deserialize_struct_field,
                deserialize_tuple,
                deserialize_enum,
                deserialize_struct, deserialize_tuple_struct
            }

            fn deserialize_ignored_any<V>(&mut self, _visitor: V) -> Result<V::Value, Self::Error>
                where V: de::Visitor,
            {
                unimplemented!()
            }

            fn deserialize<V>(&mut self, mut visitor: V) -> Result<V::Value, Error>
                where V: de::Visitor,
            {
                visitor.visit_unit()
            }

            fn deserialize_option<V>(&mut self, mut visitor: V) -> Result<V::Value, Error>
                where V: de::Visitor,
            {
                visitor.visit_none()
            }
        }

        match self.state {
            Inner if field == "$value" => {
                debug!("value");
                self.state = Done;
                match mem::replace(&mut self.members, Content::Nothing) {
                    Content::Text(s) => de::Deserialize::deserialize(&mut StringDeserializer(Some(s))),
                    Content::Nothing => de::Deserialize::deserialize(&mut UnitDeserializer),
                    Content::Members(_) => Err(Error::MissingFieldError("inner text")),
                }
            },
            Inner => if let Some(v) = self.attributes.remove(field) {
                debug!("attr");
                de::Deserialize::deserialize(&mut SeqDeserializer(v.map(Element::new_text)))
            } else if let Content::Members(ref mut m) = self.members {
                if let Some(el) = m.remove(field) {
                    debug!("el: {:?}", el);
                    de::Deserialize::deserialize(&mut SeqDeserializer(el.into_iter()))
                } else {
                    de::Deserialize::deserialize(&mut UnitDeserializer)
                }
            } else {
                de::Deserialize::deserialize(&mut UnitDeserializer)
            },
            Done => de::Deserialize::deserialize(&mut UnitDeserializer),
        }
    }
}

impl de::Deserializer for MapDeserializer {
    type Error = Error;

    de_forward_to_deserialize!{
        deserialize_bool,
        deserialize_f64, deserialize_f32,
        deserialize_u8, deserialize_u16, deserialize_u32, deserialize_u64, deserialize_usize,
        deserialize_i8, deserialize_i16, deserialize_i32, deserialize_i64, deserialize_isize,
        deserialize_char, deserialize_str, deserialize_string,
        deserialize_ignored_any,
        deserialize_bytes,
        deserialize_unit_struct, deserialize_unit,
        deserialize_seq, deserialize_seq_fixed_size,
        deserialize_map, deserialize_newtype_struct, deserialize_struct_field,
        deserialize_tuple,
        deserialize_enum,
        deserialize_struct, deserialize_tuple_struct,
        deserialize_option
    }

    fn deserialize_ignored_any<V>(&mut self, _visitor: V) -> Result<V::Value, Self::Error>
        where V: de::Visitor,
    {
        unimplemented!()
    }

    #[inline]
    fn deserialize<V>(&mut self, mut visitor: V) -> Result<V::Value, Error>
        where V: de::Visitor,
    {
        debug!("MapDeserializer!");
        visitor.visit_map(self)
    }
}
