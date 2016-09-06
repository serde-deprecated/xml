

use IsWhitespace;
use error::*;
use error::ErrorCode::*;
use serde::de;

use std::io;

macro_rules! de_forward_to_deserialize {
    ($($func:ident),*) => {
        $(de_forward_to_deserialize!{func: $func})*
    };
    (func: deserialize_unit_struct) => {
        #[inline]
        fn deserialize_unit_struct<__V>(&mut self, _: &str, visitor: __V) -> Result<__V::Value, Self::Error>
            where __V: de::Visitor {
            self.deserialize_unit(visitor)
        }
    };
    (func: deserialize_newtype_struct) => {
        #[inline]
        fn deserialize_newtype_struct<__V>(&mut self, _: &str, visitor: __V) -> Result<__V::Value, Self::Error>
            where __V: de::Visitor {
            self.deserialize(visitor)
        }
    };
    (func: deserialize_tuple) => {
        de_forward_to_deserialize!{tup_fn: deserialize_tuple}
    };
    (func: deserialize_seq_fixed_size) => {
        de_forward_to_deserialize!{tup_fn: deserialize_seq_fixed_size}
    };
    (func: deserialize_ignored_any) => {};
    (func: deserialize_tuple_struct) => {
        #[inline]
        fn deserialize_tuple_struct<__V>(&mut self, _: &str, _: usize, visitor: __V) -> Result<__V::Value, Self::Error>
            where __V: de::Visitor {
            self.deserialize_seq(visitor)
        }
    };
    (func: deserialize_struct) => {
        #[inline]
        fn deserialize_struct<__V>(&mut self, _: &str, _: &[&str], visitor: __V) -> Result<__V::Value, Self::Error>
            where __V: de::Visitor {
            self.deserialize_map(visitor)
        }
    };
    (func: deserialize_enum) => {
        #[inline]
        fn deserialize_enum<__V>(&mut self, _: &str, _: &[&str], _: __V) -> Result<__V::Value, Self::Error>
            where __V: de::EnumVisitor {
            Err(de::Error::invalid_type(de::Type::Enum))
        }
    };
    (tup_fn: $func: ident) => {
        #[inline]
        fn $func<__V>(&mut self, _: usize, visitor: __V) -> Result<__V::Value, Self::Error>
            where __V: de::Visitor {
            self.deserialize_seq(visitor)
        }
    };
    (func: deserialize_tagged_value) => {
        fn deserialize_tagged_value<__V>(&mut self) -> Result<__V, Self::Error>
            where __V: de::Deserialize {
            Err(de::Error::invalid_type(de::Type::Tagged))
        }
    };
    (func: $func:ident) => {
        #[inline]
        fn $func<__V>(&mut self, visitor: __V) -> Result<__V::Value, Self::Error>
            where __V: de::Visitor {
            self.deserialize(visitor)
        }
    };
}

mod lexer;
pub mod value;
pub use self::lexer::LexerError;
use self::lexer::Lexical::*;

macro_rules! expect {
    ($sel:expr, $pat:pat, $err:expr) => {{
        match try!($sel.bump()) {
            $pat => {},
            _ => return Err($sel.expected($err)),
        }
    }}
}

macro_rules! expect_val {
    ($sel:expr, $i:ident, $err:expr) => {{
        try!($sel.bump());
        is_val!($sel, $i, $err)
    }}
}

macro_rules! is_val {
    ($sel:expr, $i:ident, $err:expr) => {{
        match try!($sel.ch()) {
            $i(x) => x,
            _ => return Err($sel.expected($err)),
        }
    }}
}

pub struct Deserializer<Iter: Iterator<Item = io::Result<u8>>> {
    rdr: lexer::XmlIterator<Iter>,
}

pub struct InnerDeserializer<'a, Iter: Iterator<Item = io::Result<u8>> + 'a>(&'a mut lexer::XmlIterator<Iter>,
                                                                             &'a mut bool);

impl<'a, Iter: Iterator<Item = io::Result<u8>> + 'a> InnerDeserializer<'a, Iter> {
    fn decode<T>(xi: &mut lexer::XmlIterator<Iter>) -> (bool, Result<T, Error>)
        where T: de::Deserialize,
    {
        let mut is_seq = false;
        let deser = de::Deserialize::deserialize(&mut InnerDeserializer(xi, &mut is_seq));
        (is_seq, deser)
    }
    fn eat(&mut self) -> Result<(), Error> {
        debug!("InnerDeserializer::eat");
        loop {
            match try!(self.0.bump()) {
                Text(_) => {},
                StartTagName(_) => try!(self.eat_attributes()),
                EndTagName(_) => return Ok(()),
                _ => return Err(self.0.expected("tags or text")),
            }
        }
    }
    fn eat_attributes(&mut self) -> Result<(), Error> {
        debug!("InnerDeserializer::eat_attributes");
        loop {
            match try!(self.0.bump()) {
                AttributeName(_) |
                AttributeValue(_) => {},
                StartTagClose |
                EmptyElementEnd(_) => return self.eat(),
                _ => return Err(self.0.expected("attributes or tag close")),
            }
        }
    }
}

impl<'a, Iter> de::Deserializer for InnerDeserializer<'a, Iter>
    where Iter: Iterator<Item = io::Result<u8>>,
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
        deserialize_unit,
        deserialize_seq_fixed_size,
        deserialize_newtype_struct, deserialize_struct_field,
        deserialize_tuple,
        deserialize_struct
    }

    fn deserialize_ignored_any<V>(&mut self, mut visitor: V) -> Result<V::Value, Self::Error>
        where V: de::Visitor,
    {
        debug!("InnerDeserializer::deserialize_ignored_any");
        try!(self.eat());
        visitor.visit_unit()
    }

    #[inline]
    fn deserialize<V>(&mut self, mut visitor: V) -> Result<V::Value, Error>
        where V: de::Visitor,
    {
        debug!("InnerDeserializer::deserialize");
        match try!(self.0.ch()) {
            StartTagClose => {
                match {
                    let v = expect_val!(self.0, Text, "text");
                    let v = try!(self.0.check_utf8(v));
                    visitor.visit_str(v)
                } { // try! is broken sometimes
                    Ok(v) => {
                        try!(self.0.bump());
                        Ok(v)
                    },
                    Err(e) => Err(e),
                }
            },
            EmptyElementEnd(_) => visitor.visit_unit(),
            _ => Err(self.0.expected("start tag close")),
        }
    }

    fn deserialize_option<V>(&mut self, mut visitor: V) -> Result<V::Value, Error>
        where V: de::Visitor,
    {
        debug!("InnerDeserializer::deserialize_option");
        match try!(self.0.ch()) {
            EmptyElementEnd(_) => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    #[inline]
    fn deserialize_seq<V>(&mut self, mut visitor: V) -> Result<V::Value, Error>
        where V: de::Visitor,
    {
        debug!("InnerDeserializer::deserialize_seq");
        *self.1 = true;
        visitor.visit_seq(SeqVisitor::new(self.0))
    }

    fn deserialize_map<V>(&mut self, mut visitor: V) -> Result<V::Value, Error>
        where V: de::Visitor,
    {
        debug!("InnerDeserializer::deserialize_map");
        visitor.visit_map(ContentVisitor::new_attr(&mut self.0))
    }

    fn deserialize_unit_struct<V>(&mut self, _name: &str, _visitor: V) -> Result<V::Value, Error>
        where V: de::Visitor,
    {
        unimplemented!()
    }

    fn deserialize_tuple_struct<V>(&mut self, _name: &str, _len: usize, _visitor: V) -> Result<V::Value, Error>
        where V: de::Visitor,
    {
        unimplemented!()
    }

    #[inline]
    fn deserialize_enum<V>(&mut self,
                           _enum: &str,
                           _variants: &'static [&'static str],
                           mut visitor: V)
                           -> Result<V::Value, Error>
        where V: de::EnumVisitor,
    {
        debug!("InnerDeserializer::deserialize_enum");
        visitor.visit(VariantVisitor(&mut self.0))
    }
}

pub struct KeyDeserializer<'a>(&'a str);

impl<'a> KeyDeserializer<'a> {
    fn deserialize<T>(text: &str) -> Result<T, Error>
        where T: de::Deserialize,
    {
        let kds = &mut KeyDeserializer(text);
        de::Deserialize::deserialize(kds)
    }

    fn value_map<T>() -> Result<T, Error>
        where T: de::Deserialize,
    {
        let kds = &mut KeyDeserializer("$value");
        de::Deserialize::deserialize(kds)
    }
}

impl<'a> de::Deserializer for KeyDeserializer<'a> {
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
        deserialize_struct, deserialize_tuple_struct
    }

    fn deserialize_ignored_any<V>(&mut self, visitor: V) -> Result<V::Value, Self::Error>
        where V: de::Visitor,
    {
        self.deserialize(visitor)
    }

    #[inline]
    fn deserialize<V>(&mut self, mut visitor: V) -> Result<V::Value, Error>
        where V: de::Visitor,
    {
        debug!("keydeserializer::deserialize: {:#?}", self.0);
        visitor.visit_str(self.0)
    }

    #[inline]
    fn deserialize_option<V>(&mut self, mut visitor: V) -> Result<V::Value, Error>
        where V: de::Visitor,
    {
        visitor.visit_some(self)
    }

    #[inline]
    fn deserialize_enum<V>(&mut self,
                           _enum: &str,
                           _variants: &'static [&'static str],
                           _visitor: V)
                           -> Result<V::Value, Error>
        where V: de::EnumVisitor,
    {
        unimplemented!()
    }

    #[inline]
    fn deserialize_seq<V>(&mut self, _visitor: V) -> Result<V::Value, Error>
        where V: de::Visitor,
    {
        unimplemented!()
    }
}

impl<Iter> Deserializer<Iter>
    where Iter: Iterator<Item = io::Result<u8>>,
{
    /// Creates the Xml parser.
    #[inline]
    pub fn new(rdr: Iter) -> Deserializer<Iter> {
        Deserializer { rdr: lexer::XmlIterator::new(rdr) }
    }

    fn ch(&self) -> Result<lexer::Lexical, Error> {
        self.rdr.ch()
    }

    fn end(&mut self) -> Result<(), Error> {
        match try!(self.ch()) {
            EndOfFile => Ok(()),
            _ => Err(self.rdr.error(ExpectedEOF)),
        }
    }
}


impl<Iter> de::Deserializer for Deserializer<Iter>
    where Iter: Iterator<Item = io::Result<u8>>,
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
    fn deserialize<V>(&mut self, visitor: V) -> Result<V::Value, Error>
        where V: de::Visitor,
    {
        debug!("Deserializer::deserialize");
        expect!(self.rdr, StartTagName(_), "start tag name");
        try!(self.rdr.bump());
        let is_seq = &mut false;
        let v = try!(InnerDeserializer(&mut self.rdr, is_seq).deserialize(visitor));
        assert!(!*is_seq);
        match try!(self.rdr.ch()) {
            EndTagName(_) |
            EmptyElementEnd(_) => {},
            _ => return Err(self.rdr.expected("end tag")),
        }
        expect!(self.rdr, EndOfFile, "end of file");
        Ok(v)
    }

    #[inline]
    fn deserialize_option<V>(&mut self, mut visitor: V) -> Result<V::Value, Error>
        where V: de::Visitor,
    {
        debug!("Deserializer::deserialize");
        expect!(self.rdr, StartTagName(_), "start tag name");
        let is_seq = &mut false;
        let v = match try!(self.rdr.bump()) {
            StartTagClose => visitor.visit_some(&mut InnerDeserializer(&mut self.rdr, is_seq)),
            EmptyElementEnd(_) => visitor.visit_none(),
            _ => Err(self.rdr.expected("start tag close")),
        };
        let v = try!(v);
        assert!(!*is_seq);
        match try!(self.rdr.ch()) {
            EndTagName(_) |
            EmptyElementEnd(_) => {},
            _ => return Err(self.rdr.expected("end tag")),
        }
        expect!(self.rdr, EndOfFile, "end of file");
        Ok(v)
    }

    #[inline]
    fn deserialize_enum<V>(&mut self,
                           _enum: &str,
                           _variants: &'static [&'static str],
                           mut visitor: V)
                           -> Result<V::Value, Error>
        where V: de::EnumVisitor,
    {
        expect!(self.rdr, StartTagName(_), "start tag name");
        try!(self.rdr.bump());
        let v = visitor.visit(VariantVisitor(&mut self.rdr));
        let v = try!(v);
        expect!(self.rdr, EndOfFile, "end of file");
        Ok(v)
    }

    #[inline]
    fn deserialize_seq<V>(&mut self, _visitor: V) -> Result<V::Value, Error>
        where V: de::Visitor,
    {
        unimplemented!()
    }

    #[inline]
    fn deserialize_map<V>(&mut self, visitor: V) -> Result<V::Value, Error>
        where V: de::Visitor,
    {
        debug!("Deserializer::deserialize_map");
        expect!(self.rdr, StartTagName(_), "start tag name"); // TODO: named map
        try!(self.rdr.bump());
        let is_seq = &mut false;
        let v = try!(InnerDeserializer(&mut self.rdr, is_seq).deserialize_map(visitor));
        assert!(!*is_seq);
        match try!(self.ch()) {
            EndTagName(_) |
            EmptyElementEnd(_) => {},
            _ => return Err(self.rdr.expected("end tag")),
        }
        expect!(self.rdr, EndOfFile, "end of file");
        Ok(v)
    }
}

struct VariantVisitor<'a, Iter: Iterator<Item = io::Result<u8>> + 'a>(&'a mut lexer::XmlIterator<Iter>);

impl<'a, Iter: 'a> de::VariantVisitor for VariantVisitor<'a, Iter>
    where Iter: Iterator<Item = io::Result<u8>>,
{
    type Error = Error;

    fn visit_variant<V>(&mut self) -> Result<V, Self::Error>
        where V: de::Deserialize,
    {
        if b"xsi:type" != is_val!(self.0, AttributeName, "attribute name") {
            return Err(self.0.error(Expected("attribute xsi:type")));
        }
        let v = expect_val!(self.0, AttributeValue, "attribute value");
        let v = try!(self.0.check_utf8(v));
        KeyDeserializer::deserialize(v)
    }

    /// `visit_unit` is called when deserializing a variant with no values.
    fn visit_unit(&mut self) -> Result<(), Self::Error> {
        expect!(self.0, EmptyElementEnd(_), "empty element end");
        Ok(())
    }

    /// `visit_tuple` is called when deserializing a tuple-like variant.
    fn visit_tuple<V>(&mut self, _len: usize, _visitor: V) -> Result<V::Value, Self::Error>
        where V: de::Visitor,
    {
        unimplemented!()
    }

    /// `visit_struct` is called when deserializing a struct-like variant.
    fn visit_struct<V>(&mut self, _fields: &'static [&'static str], mut visitor: V) -> Result<V::Value, Self::Error>
        where V: de::Visitor,
    {
        try!(self.0.bump());
        visitor.visit_map(ContentVisitor::new_attr(&mut self.0))
    }

    /// `visit_newtype` is called when deseriailizing a variant with a single value.
    fn visit_newtype<D>(&mut self) -> Result<D, Self::Error>
        where D: de::Deserialize,
    {
        expect!(self.0, StartTagClose, "start tag close");
        struct Dummy<'a, Iter: Iterator<Item = io::Result<u8>> + 'a>(&'a mut lexer::XmlIterator<Iter>);

        impl<'a, Iter: Iterator<Item = io::Result<u8>> + 'a> de::Deserializer for Dummy<'a, Iter> {
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
                deserialize_enum,
                deserialize_struct, deserialize_tuple_struct,
                deserialize_option
            }

            fn deserialize<V>(&mut self, _visitor: V) -> Result<V::Value, Error>
                where V: de::Visitor,
            {
                let ret = {
                    let v = expect_val!(self.0, Text, "content");
                    let v = try!(self.0.check_utf8(v));
                    try!(KeyDeserializer::deserialize(v))
                };
                expect!(self.0, EndTagName(_), "end tag name");
                Ok(ret)
            }

            fn deserialize_map<V>(&mut self, mut visitor: V) -> Result<V::Value, Error>
                where V: de::Visitor,
            {
                visitor.visit_map(ContentVisitor::new_attr(&mut self.0))
            }

            fn deserialize_ignored_any<V>(&mut self, _visitor: V) -> Result<V::Value, Self::Error>
                where V: de::Visitor,
            {
                unimplemented!()
            }
        }
        de::Deserialize::deserialize(&mut Dummy(&mut self.0))
    }
}

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
        deserialize_seq_fixed_size,
        deserialize_newtype_struct, deserialize_struct_field,
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

    fn deserialize_seq<V>(&mut self, mut visitor: V) -> Result<V::Value, Error>
        where V: de::Visitor,
    {
        visitor.visit_seq(EmptySeqVisitor)
    }

    fn deserialize_map<V>(&mut self, mut visitor: V) -> Result<V::Value, Error>
        where V: de::Visitor,
    {
        visitor.visit_map(EmptyMapVisitor)
    }
}

struct EmptySeqVisitor;
impl de::SeqVisitor for EmptySeqVisitor {
    type Error = Error;

    fn visit<T>(&mut self) -> Result<Option<T>, Error>
        where T: de::Deserialize,
    {
        Ok(None)
    }

    fn end(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

struct EmptyMapVisitor;
impl de::MapVisitor for EmptyMapVisitor {
    type Error = Error;

    fn visit_key<K>(&mut self) -> Result<Option<K>, Error>
        where K: de::Deserialize,
    {
        Ok(None)
    }

    fn visit_value<V>(&mut self) -> Result<V, Error>
        where V: de::Deserialize,
    {
        unreachable!()
    }

    fn end(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn missing_field<V>(&mut self, _field: &'static str) -> Result<V, Error>
        where V: de::Deserialize,
    {
        Ok(try!(de::Deserialize::deserialize(&mut UnitDeserializer)))
    }
}

struct ContentVisitor<'a, Iter: 'a>
    where Iter: Iterator<Item = io::Result<u8>>,
{
    de: &'a mut lexer::XmlIterator<Iter>,
    state: ContentVisitorState,
}

#[derive(Debug)]
enum ContentVisitorState {
    Attribute,
    Element,
    Value,
    Inner,
}

impl<'a, Iter> ContentVisitor<'a, Iter>
    where Iter: Iterator<Item = io::Result<u8>>,
{
    fn new_attr(de: &'a mut lexer::XmlIterator<Iter>) -> Self {
        ContentVisitor {
            de: de,
            state: ContentVisitorState::Attribute,
        }
    }
}

impl<'a, Iter> de::MapVisitor for ContentVisitor<'a, Iter>
    where Iter: Iterator<Item = io::Result<u8>>,
{
    type Error = Error;

    fn visit_key<K>(&mut self) -> Result<Option<K>, Error>
        where K: de::Deserialize,
    {
        use self::ContentVisitorState::*;
        debug!("visit_key: {:?}", (&self.state, try!(self.de.ch())));
        match match (&self.state, try!(self.de.ch())) {
            (&Attribute, EmptyElementEnd(_)) => return Ok(None),
            (&Attribute, StartTagClose) => 0,
            (&Attribute, AttributeName(n)) =>
                return Ok(Some(try!(KeyDeserializer::deserialize(try!(self.de.check_utf8(n)))))),
            (&Element, StartTagName(n)) =>
                return Ok(Some(try!(KeyDeserializer::deserialize(try!(self.de.check_utf8(n)))))),
            (&Inner, Text(_)) => 1,
            (&Inner, _) => 4,
            (&Value, EndTagName(_)) => return Ok(None),
            (&Value, Text(txt)) if txt.is_ws() => 3,
            (&Value, Text(_)) => return Ok(Some(try!(KeyDeserializer::value_map()))),
            (&Element, EmptyElementEnd(_)) => 2,
            (&Element, Text(txt)) if txt.is_ws() => 5,
            (&Element, EndTagName(_)) => return Ok(None),
            (&Element, EndOfFile) => return Ok(None),
            (&Element, Text(_)) => return Err(self.de.error(NonWhitespaceBetweenElements)),
            _ => panic!("unimplemented: {:?}", (&self.state, try!(self.de.ch()))),
        } {
            0 => {
                // hack for Attribute, StartTagClose
                try!(self.de.bump());
                self.state = Inner;
                self.visit_key()
            },
            1 => {
                // hack for Inner, Text
                self.state = Value;
                self.visit_key()
            },
            2 | 5 => {
                // hack for Element, EmptyElementEnd
                // happens when coming out of an empty element which is an inner value
                // maybe catch in visit_value?
                try!(self.de.bump());
                self.visit_key()
            },
            3 => match KeyDeserializer::value_map() {
                Err(Error::UnknownField(_)) => {
                    try!(self.de.bump());
                    Ok(None)
                },
                Err(e) => Err(e),
                Ok(x) => Ok(Some(x)),
            },
            4 => {
                self.state = Element;
                self.visit_key()
            },
            _ => unreachable!(),
        }
    }

    fn visit_value<V>(&mut self) -> Result<V, Error>
        where V: de::Deserialize,
    {
        use self::ContentVisitorState::*;
        debug!("visit_value: {:?}", &self.state);
        match self.state {
            Attribute => {
                let v = {
                    let v = expect_val!(self.de, AttributeValue, "attribute value");
                    let v = try!(self.de.check_utf8(v));
                    try!(KeyDeserializer::deserialize(v))
                };
                try!(self.de.bump());
                Ok(v)
            },
            Element => {
                try!(self.de.bump());
                let (is_seq, v) = InnerDeserializer::decode(&mut self.de);
                let v = try!(v);
                debug!("is_seq: {}", is_seq);
                if !is_seq {
                    match try!(self.de.ch()) {
                        EmptyElementEnd(_) |
                        EndTagName(_) => {},
                        _ => return Err(self.de.expected("tag close")),
                    }
                    try!(self.de.bump());
                }
                Ok(v)
            },
            Value => {
                let v = {
                    let v = is_val!(self.de, Text, "text");
                    let v = try!(self.de.check_utf8(v));
                    try!(KeyDeserializer::deserialize(v))
                };
                try!(self.de.bump());
                Ok(v)
            },
            Inner => unreachable!(),
        }
    }

    fn end(&mut self) -> Result<(), Error> {
        debug!("end: {:?}", &self.state);
        Ok(())
    }

    fn missing_field<V>(&mut self, field: &'static str) -> Result<V, Error>
        where V: de::Deserialize,
    {
        debug!("missing field: {}", field);
        // See if the type can deserialize from a unit.
        de::Deserialize::deserialize(&mut UnitDeserializer)
    }
}

struct SeqVisitor<'a, Iter: 'a + Iterator<Item = io::Result<u8>>> {
    de: &'a mut lexer::XmlIterator<Iter>,
    done: bool,
}

impl<'a, Iter> SeqVisitor<'a, Iter>
    where Iter: Iterator<Item = io::Result<u8>>,
{
    fn new(de: &'a mut lexer::XmlIterator<Iter>) -> Self {
        SeqVisitor {
            de: de,
            done: false,
        }
    }
}

impl<'a, Iter> de::SeqVisitor for SeqVisitor<'a, Iter>
    where Iter: Iterator<Item = io::Result<u8>>,
{
    type Error = Error;

    fn visit<T>(&mut self) -> Result<Option<T>, Error>
        where T: de::Deserialize,
    {
        debug!("SeqVisitor::visit: {:?}", (self.done, self.de.ch()));
        if self.done {
            return Ok(None);
        }
        let (is_seq, v) = InnerDeserializer::decode(&mut self.de);
        let v = try!(v);
        if is_seq {
            return Err(self.de.error(XmlDoesntSupportSeqofSeq));
        }
        match try!(self.de.ch()) {
            EndTagName(_) |
            EmptyElementEnd(_) => {},
            _ => return Err(self.de.expected("end tag")),
        }
        self.de.stash();
        try!(self.de.bump());
        // cannot match on bump here due to rust-bug in functions
        // with &mut self arg and & return value
        match match try!(self.de.ch()) {
            StartTagName(n) if n == self.de.stash_view() => 0,
            StartTagName(_) => 1,
            Text(txt) if txt.is_ws() => 2,
            _ => unimplemented!(),
        } {
            0 => {
                try!(self.de.bump());
            },
            1 => self.done = true,
            2 => match try!(self.de.bump()) {
                EndTagName(_) => self.done = true,
                _ => unimplemented!(),
            },
            _ => unreachable!(),
        }
        Ok(Some(v))
    }

    fn end(&mut self) -> Result<(), Error> {
        debug!("SeqVisitor::end");
        Ok(())
    }
}

/// Decodes an xml value from an `Iterator<u8>`.
pub fn from_iter<I, T>(iter: I) -> Result<T, Error>
    where I: Iterator<Item = io::Result<u8>>,
          T: de::Deserialize,
{
    let mut de = Deserializer::new(iter);
    let value = try!(de::Deserialize::deserialize(&mut de));

    // Make sure the whole stream has been consumed.
    try!(de.end());
    Ok(value)
}

/// Decodes an xml value from a string
pub fn from_str<T>(s: &str) -> Result<T, Error>
    where T: de::Deserialize,
{
    from_iter(s.bytes().map(Ok))
}
