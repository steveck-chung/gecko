/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#![allow(non_camel_case_types)]

pub use cssparser::RGBA;


macro_rules! define_css_keyword_enum {
    ($name: ident: $( $css: expr => $variant: ident ),+,) => {
        define_css_keyword_enum!($name: $( $css => $variant ),+);
    };
    ($name: ident: $( $css: expr => $variant: ident ),+) => {
        #[allow(non_camel_case_types)]
        #[derive(Clone, Eq, PartialEq, FromPrimitive, Copy)]
        pub enum $name {
            $( $variant ),+
        }

        impl $name {
            pub fn parse(input: &mut ::cssparser::Parser) -> Result<$name, ()> {
                match_ignore_ascii_case! { try!(input.expect_ident()),
                    $( $css => Ok($name::$variant) ),+
                    _ => Err(())
                }
            }
        }

        impl ::std::fmt::Show for $name {
            #[inline]
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                use cssparser::ToCss;
                self.fmt_to_css(f)
            }
        }

        impl ::cssparser::ToCss for $name {
            fn to_css<W>(&self, dest: &mut W) -> ::text_writer::Result
            where W: ::text_writer::TextWriter {
                match self {
                    $( &$name::$variant => dest.write_str($css) ),+
                }
            }
        }
    }
}


pub type CSSFloat = f64;

pub mod specified {
    use std::ascii::AsciiExt;
    use std::f64::consts::PI;
    use std::fmt;
    use std::fmt::{Formatter, Show};
    use url::Url;
    use cssparser::{self, Token, Parser, ToCss, CssStringWriter};
    use parser::ParserContext;
    use text_writer::{self, TextWriter};
    use util::geometry::Au;
    use super::CSSFloat;
    use super::computed;

    #[derive(Clone, PartialEq)]
    pub struct CSSColor {
        pub parsed: cssparser::Color,
        pub authored: Option<String>,
    }
    impl CSSColor {
        pub fn parse(input: &mut Parser) -> Result<CSSColor, ()> {
            let start_position = input.position();
            let authored = match input.next() {
                Ok(Token::Ident(s)) => Some(s.into_owned()),
                _ => None,
            };
            input.reset(start_position);
            Ok(CSSColor {
                parsed: try!(cssparser::Color::parse(input)),
                authored: authored,
            })
        }
    }

    impl fmt::Show for CSSColor {
        #[inline] fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.fmt_to_css(f) }
    }

    impl ToCss for CSSColor {
        fn to_css<W>(&self, dest: &mut W) -> text_writer::Result where W: TextWriter {
            match self.authored {
                Some(ref s) => dest.write_str(s.as_slice()),
                None => self.parsed.to_css(dest),
            }
        }
    }

    #[derive(Clone, PartialEq)]
    pub struct CSSRGBA {
        pub parsed: cssparser::RGBA,
        pub authored: Option<String>,
    }
    impl fmt::Show for CSSRGBA {
        #[inline] fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.fmt_to_css(f) }
    }

    impl ToCss for CSSRGBA {
        fn to_css<W>(&self, dest: &mut W) -> text_writer::Result where W: TextWriter {
            match self.authored {
                Some(ref s) => dest.write_str(s.as_slice()),
                None => self.parsed.to_css(dest),
            }
        }
    }

    #[derive(Clone, PartialEq)]
    pub struct CSSImage(pub Option<Image>);

    impl fmt::Show for CSSImage {
        #[inline] fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.fmt_to_css(f) }
    }

    impl ToCss for CSSImage {
        fn to_css<W>(&self, dest: &mut W) -> text_writer::Result where W: TextWriter {
            match self {
                &CSSImage(Some(ref image)) => image.to_css(dest),
                &CSSImage(None) => dest.write_str("none"),
            }
        }
    }

    #[derive(Clone, PartialEq, Copy)]
    pub enum Length {
        Au(Au),  // application units
        Em(CSSFloat),
        Ex(CSSFloat),
        Rem(CSSFloat),

        /// HTML5 "character width", as defined in HTML5 § 14.5.4.
        ///
        /// This cannot be specified by the user directly and is only generated by
        /// `Stylist::synthesize_rules_for_legacy_attributes()`.
        ServoCharacterWidth(i32),
    }

    impl fmt::Show for Length {
        #[inline] fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.fmt_to_css(f) }
    }

    impl ToCss for Length {
        fn to_css<W>(&self, dest: &mut W) -> text_writer::Result where W: TextWriter {
            match self {
                &Length::Au(length) => write!(dest, "{}px", length.to_subpx()),
                &Length::Em(length) => write!(dest, "{}em", length),
                &Length::Ex(length) => write!(dest, "{}ex", length),
                &Length::Rem(length) => write!(dest, "{}rem", length),
                &Length::ServoCharacterWidth(_)
                => panic!("internal CSS values should never be serialized"),
            }
        }
    }

    const AU_PER_PX: CSSFloat = 60.;
    const AU_PER_IN: CSSFloat = AU_PER_PX * 96.;
    const AU_PER_CM: CSSFloat = AU_PER_IN / 2.54;
    const AU_PER_MM: CSSFloat = AU_PER_IN / 25.4;
    const AU_PER_PT: CSSFloat = AU_PER_IN / 72.;
    const AU_PER_PC: CSSFloat = AU_PER_PT * 12.;
    impl Length {
        #[inline]
        fn parse_internal(input: &mut Parser, negative_ok: bool) -> Result<Length, ()> {
            match try!(input.next()) {
                Token::Dimension(ref value, ref unit) if negative_ok || value.value >= 0. => {
                    Length::parse_dimension(value.value, unit.as_slice())
                }
                Token::Number(ref value) if value.value == 0. => Ok(Length::Au(Au(0))),
                _ => Err(())
            }
        }
        #[allow(dead_code)]
        pub fn parse(input: &mut Parser) -> Result<Length, ()> {
            Length::parse_internal(input, /* negative_ok = */ true)
        }
        pub fn parse_non_negative(input: &mut Parser) -> Result<Length, ()> {
            Length::parse_internal(input, /* negative_ok = */ false)
        }
        pub fn parse_dimension(value: CSSFloat, unit: &str) -> Result<Length, ()> {
            match_ignore_ascii_case! { unit,
                "px" => Ok(Length::from_px(value)),
                "in" => Ok(Length::Au(Au((value * AU_PER_IN) as i32))),
                "cm" => Ok(Length::Au(Au((value * AU_PER_CM) as i32))),
                "mm" => Ok(Length::Au(Au((value * AU_PER_MM) as i32))),
                "pt" => Ok(Length::Au(Au((value * AU_PER_PT) as i32))),
                "pc" => Ok(Length::Au(Au((value * AU_PER_PC) as i32))),
                "em" => Ok(Length::Em(value)),
                "ex" => Ok(Length::Ex(value)),
                "rem" => Ok(Length::Rem(value))
                _ => Err(())
            }
        }
        #[inline]
        pub fn from_px(px_value: CSSFloat) -> Length {
            Length::Au(Au((px_value * AU_PER_PX) as i32))
        }
    }


    #[derive(Clone, PartialEq, Copy)]
    pub enum LengthOrPercentage {
        Length(Length),
        Percentage(CSSFloat),  // [0 .. 100%] maps to [0.0 .. 1.0]
    }

    impl fmt::Show for LengthOrPercentage {
        #[inline] fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.fmt_to_css(f) }
    }

    impl ToCss for LengthOrPercentage {
        fn to_css<W>(&self, dest: &mut W) -> text_writer::Result where W: TextWriter {
            match self {
                &LengthOrPercentage::Length(length) => length.to_css(dest),
                &LengthOrPercentage::Percentage(percentage)
                => write!(dest, "{}%", percentage * 100.),
            }
        }
    }
    impl LengthOrPercentage {
        fn parse_internal(input: &mut Parser, negative_ok: bool)
                          -> Result<LengthOrPercentage, ()> {
            match try!(input.next()) {
                Token::Dimension(ref value, ref unit) if negative_ok || value.value >= 0. => {
                    Length::parse_dimension(value.value, unit.as_slice())
                    .map(LengthOrPercentage::Length)
                }
                Token::Percentage(ref value) if negative_ok || value.unit_value >= 0. => {
                    Ok(LengthOrPercentage::Percentage(value.unit_value))
                }
                Token::Number(ref value) if value.value == 0. => {
                    Ok(LengthOrPercentage::Length(Length::Au(Au(0))))
                }
                _ => Err(())
            }
        }
        #[allow(dead_code)]
        #[inline]
        pub fn parse(input: &mut Parser) -> Result<LengthOrPercentage, ()> {
            LengthOrPercentage::parse_internal(input, /* negative_ok = */ true)
        }
        #[inline]
        pub fn parse_non_negative(input: &mut Parser) -> Result<LengthOrPercentage, ()> {
            LengthOrPercentage::parse_internal(input, /* negative_ok = */ false)
        }
    }

    #[derive(Clone, PartialEq, Copy)]
    pub enum LengthOrPercentageOrAuto {
        Length(Length),
        Percentage(CSSFloat),  // [0 .. 100%] maps to [0.0 .. 1.0]
        Auto,
    }

    impl fmt::Show for LengthOrPercentageOrAuto {
        #[inline] fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.fmt_to_css(f) }
    }

    impl ToCss for LengthOrPercentageOrAuto {
        fn to_css<W>(&self, dest: &mut W) -> text_writer::Result where W: TextWriter {
            match self {
                &LengthOrPercentageOrAuto::Length(length) => length.to_css(dest),
                &LengthOrPercentageOrAuto::Percentage(percentage)
                => write!(dest, "{}%", percentage * 100.),
                &LengthOrPercentageOrAuto::Auto => dest.write_str("auto"),
            }
        }
    }
    impl LengthOrPercentageOrAuto {
        fn parse_internal(input: &mut Parser, negative_ok: bool)
                     -> Result<LengthOrPercentageOrAuto, ()> {
            match try!(input.next()) {
                Token::Dimension(ref value, ref unit) if negative_ok || value.value >= 0. => {
                    Length::parse_dimension(value.value, unit.as_slice())
                    .map(LengthOrPercentageOrAuto::Length)
                }
                Token::Percentage(ref value) if negative_ok || value.unit_value >= 0. => {
                    Ok(LengthOrPercentageOrAuto::Percentage(value.unit_value))
                }
                Token::Number(ref value) if value.value == 0. => {
                    Ok(LengthOrPercentageOrAuto::Length(Length::Au(Au(0))))
                }
                Token::Ident(ref value) if value.eq_ignore_ascii_case("auto") => {
                    Ok(LengthOrPercentageOrAuto::Auto)
                }
                _ => Err(())
            }
        }
        #[inline]
        pub fn parse(input: &mut Parser) -> Result<LengthOrPercentageOrAuto, ()> {
            LengthOrPercentageOrAuto::parse_internal(input, /* negative_ok = */ true)
        }
        #[inline]
        pub fn parse_non_negative(input: &mut Parser) -> Result<LengthOrPercentageOrAuto, ()> {
            LengthOrPercentageOrAuto::parse_internal(input, /* negative_ok = */ false)
        }
    }

    #[derive(Clone, PartialEq, Copy)]
    pub enum LengthOrPercentageOrNone {
        Length(Length),
        Percentage(CSSFloat),  // [0 .. 100%] maps to [0.0 .. 1.0]
        None,
    }

    impl fmt::Show for LengthOrPercentageOrNone {
        #[inline] fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.fmt_to_css(f) }
    }

    impl ToCss for LengthOrPercentageOrNone {
        fn to_css<W>(&self, dest: &mut W) -> text_writer::Result where W: TextWriter {
            match self {
                &LengthOrPercentageOrNone::Length(length) => length.to_css(dest),
                &LengthOrPercentageOrNone::Percentage(percentage)
                => write!(dest, "{}%", percentage * 100.),
                &LengthOrPercentageOrNone::None => dest.write_str("none"),
            }
        }
    }
    impl LengthOrPercentageOrNone {
        fn parse_internal(input: &mut Parser, negative_ok: bool)
                     -> Result<LengthOrPercentageOrNone, ()> {
            match try!(input.next()) {
                Token::Dimension(ref value, ref unit) if negative_ok || value.value >= 0. => {
                    Length::parse_dimension(value.value, unit.as_slice())
                    .map(LengthOrPercentageOrNone::Length)
                }
                Token::Percentage(ref value) if negative_ok || value.unit_value >= 0. => {
                    Ok(LengthOrPercentageOrNone::Percentage(value.unit_value))
                }
                Token::Number(ref value) if value.value == 0. => {
                    Ok(LengthOrPercentageOrNone::Length(Length::Au(Au(0))))
                }
                Token::Ident(ref value) if value.eq_ignore_ascii_case("none") => {
                    Ok(LengthOrPercentageOrNone::None)
                }
                _ => Err(())
            }
        }
        #[allow(dead_code)]
        #[inline]
        pub fn parse(input: &mut Parser) -> Result<LengthOrPercentageOrNone, ()> {
            LengthOrPercentageOrNone::parse_internal(input, /* negative_ok = */ true)
        }
        #[inline]
        pub fn parse_non_negative(input: &mut Parser) -> Result<LengthOrPercentageOrNone, ()> {
            LengthOrPercentageOrNone::parse_internal(input, /* negative_ok = */ false)
        }
    }

    // http://dev.w3.org/csswg/css2/colors.html#propdef-background-position
    #[derive(Clone, PartialEq, Copy)]
    pub enum PositionComponent {
        Length(Length),
        Percentage(CSSFloat),  // [0 .. 100%] maps to [0.0 .. 1.0]
        Center,
        Left,
        Right,
        Top,
        Bottom,
    }
    impl PositionComponent {
        pub fn parse(input: &mut Parser) -> Result<PositionComponent, ()> {
            match try!(input.next()) {
                Token::Dimension(ref value, ref unit) => {
                    Length::parse_dimension(value.value, unit.as_slice())
                    .map(PositionComponent::Length)
                }
                Token::Percentage(ref value) => {
                    Ok(PositionComponent::Percentage(value.unit_value))
                }
                Token::Number(ref value) if value.value == 0. => {
                    Ok(PositionComponent::Length(Length::Au(Au(0))))
                }
                Token::Ident(value) => {
                    match_ignore_ascii_case! { value,
                        "center" => Ok(PositionComponent::Center),
                        "left" => Ok(PositionComponent::Left),
                        "right" => Ok(PositionComponent::Right),
                        "top" => Ok(PositionComponent::Top),
                        "bottom" => Ok(PositionComponent::Bottom)
                        _ => Err(())
                    }
                }
                _ => Err(())
            }
        }
        #[inline]
        pub fn to_length_or_percentage(self) -> LengthOrPercentage {
            match self {
                PositionComponent::Length(x) => LengthOrPercentage::Length(x),
                PositionComponent::Percentage(x) => LengthOrPercentage::Percentage(x),
                PositionComponent::Center => LengthOrPercentage::Percentage(0.5),
                PositionComponent::Left |
                PositionComponent::Top => LengthOrPercentage::Percentage(0.0),
                PositionComponent::Right |
                PositionComponent::Bottom => LengthOrPercentage::Percentage(1.0),
            }
        }
    }

    #[derive(Clone, PartialEq, PartialOrd, Copy)]
    pub struct Angle(pub CSSFloat);

    impl fmt::Show for Angle {
        #[inline] fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.fmt_to_css(f) }
    }

    impl ToCss for Angle {
        fn to_css<W>(&self, dest: &mut W) -> text_writer::Result where W: TextWriter {
            let Angle(value) = *self;
            write!(dest, "{}rad", value)
        }
    }

    impl Angle {
        pub fn radians(self) -> f64 {
            let Angle(radians) = self;
            radians
        }
    }

    const RAD_PER_DEG: CSSFloat = PI / 180.0;
    const RAD_PER_GRAD: CSSFloat = PI / 200.0;
    const RAD_PER_TURN: CSSFloat = PI * 2.0;

    impl Angle {
        /// Parses an angle according to CSS-VALUES § 6.1.
        pub fn parse(input: &mut Parser) -> Result<Angle, ()> {
            match try!(input.next()) {
                Token::Dimension(value, unit) => {
                    match_ignore_ascii_case! { unit,
                        "deg" => Ok(Angle(value.value * RAD_PER_DEG)),
                        "grad" => Ok(Angle(value.value * RAD_PER_GRAD)),
                        "turn" => Ok(Angle(value.value * RAD_PER_TURN)),
                        "rad" => Ok(Angle(value.value))
                        _ => Err(())
                    }
                }
                Token::Number(ref value) if value.value == 0. => Ok(Angle(0.)),
                _ => Err(())
            }
        }
    }

    /// Specified values for an image according to CSS-IMAGES.
    #[derive(Clone, PartialEq)]
    pub enum Image {
        Url(Url),
        LinearGradient(LinearGradient),
    }

    impl fmt::Show for Image {
        #[inline] fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.fmt_to_css(f) }
    }

    impl ToCss for Image {
        fn to_css<W>(&self, dest: &mut W) -> text_writer::Result where W: TextWriter {
            match self {
                &Image::Url(ref url) => {
                    try!(dest.write_str("url(\""));
                    try!(write!(&mut CssStringWriter::new(dest), "{}", url));
                    try!(dest.write_str("\")"));
                    Ok(())
                }
                &Image::LinearGradient(ref gradient) => gradient.to_css(dest)
            }
        }
    }

    impl Image {
        pub fn parse(context: &ParserContext, input: &mut Parser) -> Result<Image, ()> {
            match try!(input.next()) {
                Token::Url(url) => {
                    Ok(Image::Url(context.parse_url(url.as_slice())))
                }
                Token::Function(name) => {
                    match_ignore_ascii_case! { name,
                        "linear-gradient" => {
                            Ok(Image::LinearGradient(try!(
                                input.parse_nested_block(LinearGradient::parse_function))))
                        }
                        _ => Err(())
                    }
                }
                _ => Err(())
            }
        }

        pub fn to_computed_value(self, context: &computed::Context) -> computed::Image {
            match self {
                Image::Url(url) => computed::Image::Url(url),
                Image::LinearGradient(linear_gradient) => {
                    computed::Image::LinearGradient(
                        computed::LinearGradient::compute(linear_gradient, context))
                }
            }
        }
    }

    /// Specified values for a CSS linear gradient.
    #[derive(Clone, PartialEq)]
    pub struct LinearGradient {
        /// The angle or corner of the gradient.
        pub angle_or_corner: AngleOrCorner,

        /// The color stops.
        pub stops: Vec<ColorStop>,
    }

    impl fmt::Show for LinearGradient {
        #[inline] fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.fmt_to_css(f) }
    }

    impl ToCss for LinearGradient {
        fn to_css<W>(&self, dest: &mut W) -> text_writer::Result where W: TextWriter {
            try!(dest.write_str("linear-gradient("));
            try!(self.angle_or_corner.to_css(dest));
            for stop in self.stops.iter() {
                try!(dest.write_str(", "));
                try!(stop.to_css(dest));
            }
            try!(dest.write_char(')'));
            Ok(())
        }
    }

    /// Specified values for an angle or a corner in a linear gradient.
    #[derive(Clone, PartialEq, Copy)]
    pub enum AngleOrCorner {
        Angle(Angle),
        Corner(HorizontalDirection, VerticalDirection),
    }

    impl fmt::Show for AngleOrCorner {
        #[inline] fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.fmt_to_css(f) }
    }

    impl ToCss for AngleOrCorner {
        fn to_css<W>(&self, dest: &mut W) -> text_writer::Result where W: TextWriter {
            match self {
                &AngleOrCorner::Angle(angle) => angle.to_css(dest),
                &AngleOrCorner::Corner(horizontal, vertical) => {
                    try!(dest.write_str("to "));
                    try!(horizontal.to_css(dest));
                    try!(dest.write_char(' '));
                    try!(vertical.to_css(dest));
                    Ok(())
                }
            }
        }
    }

    /// Specified values for one color stop in a linear gradient.
    #[derive(Clone, PartialEq)]
    pub struct ColorStop {
        /// The color of this stop.
        pub color: CSSColor,

        /// The position of this stop. If not specified, this stop is placed halfway between the
        /// point that precedes it and the point that follows it.
        pub position: Option<LengthOrPercentage>,
    }

    impl fmt::Show for ColorStop {
        #[inline] fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.fmt_to_css(f) }
    }

    impl ToCss for ColorStop {
        fn to_css<W>(&self, dest: &mut W) -> text_writer::Result where W: TextWriter {
            try!(self.color.to_css(dest));
            if let Some(position) = self.position {
                try!(dest.write_char(' '));
                try!(position.to_css(dest));
            }
            Ok(())
        }
    }

    define_css_keyword_enum!(HorizontalDirection: "left" => Left, "right" => Right);
    define_css_keyword_enum!(VerticalDirection: "top" => Top, "bottom" => Bottom);

    fn parse_one_color_stop(input: &mut Parser) -> Result<ColorStop, ()> {
        Ok(ColorStop {
            color: try!(CSSColor::parse(input)),
            position: input.try(LengthOrPercentage::parse).ok(),
        })
    }

    impl LinearGradient {
        /// Parses a linear gradient from the given arguments.
        pub fn parse_function(input: &mut Parser) -> Result<LinearGradient, ()> {
            let angle_or_corner = if input.try(|input| input.expect_ident_matching("to")).is_ok() {
                let (horizontal, vertical) =
                if let Ok(value) = input.try(HorizontalDirection::parse) {
                    (Some(value), input.try(VerticalDirection::parse).ok())
                } else {
                    let value = try!(VerticalDirection::parse(input));
                    (input.try(HorizontalDirection::parse).ok(), Some(value))
                };
                try!(input.expect_comma());
                match (horizontal, vertical) {
                    (None, Some(VerticalDirection::Top)) => {
                        AngleOrCorner::Angle(Angle(0.0))
                    },
                    (Some(HorizontalDirection::Right), None) => {
                        AngleOrCorner::Angle(Angle(PI * 0.5))
                    },
                    (None, Some(VerticalDirection::Bottom)) => {
                        AngleOrCorner::Angle(Angle(PI))
                    },
                    (Some(HorizontalDirection::Left), None) => {
                        AngleOrCorner::Angle(Angle(PI * 1.5))
                    },
                    (Some(horizontal), Some(vertical)) => {
                        AngleOrCorner::Corner(horizontal, vertical)
                    }
                    (None, None) => unreachable!(),
                }
            } else if let Ok(angle) = input.try(Angle::parse) {
                try!(input.expect_comma());
                AngleOrCorner::Angle(angle)
            } else {
                AngleOrCorner::Angle(Angle(PI))
            };
            // Parse the color stops.
            let stops = try!(input.parse_comma_separated(parse_one_color_stop));
            if stops.len() < 2 {
                return Err(())
            }
            Ok(LinearGradient {
                angle_or_corner: angle_or_corner,
                stops: stops,
            })
        }
    }


    pub fn parse_border_width(input: &mut Parser) -> Result<Length, ()> {
        input.try(Length::parse_non_negative).or_else(|()| {
            match_ignore_ascii_case! { try!(input.expect_ident()),
                "thin" => Ok(Length::from_px(1.)),
                "medium" => Ok(Length::from_px(3.)),
                "thick" => Ok(Length::from_px(5.))
                _ => Err(())
            }
        })
    }

    define_css_keyword_enum! { BorderStyle:
        "none" => none,
        "solid" => solid,
        "double" => double,
        "dotted" => dotted,
        "dashed" => dashed,
        "hidden" => hidden,
        "groove" => groove,
        "ridge" => ridge,
        "inset" => inset,
        "outset" => outset,
    }
}


pub mod computed {
    pub use super::specified::BorderStyle;
    use super::specified::{AngleOrCorner};
    use super::{specified, CSSFloat};
    pub use cssparser::Color as CSSColor;
    use properties::longhands;
    use std::fmt;
    use url::Url;
    use util::geometry::Au;

    #[allow(missing_copy_implementations)]  // It’s kinda big
    pub struct Context {
        pub inherited_font_weight: longhands::font_weight::computed_value::T,
        pub inherited_font_size: longhands::font_size::computed_value::T,
        pub inherited_text_decorations_in_effect:
            longhands::_servo_text_decorations_in_effect::computed_value::T,
        pub inherited_height: longhands::height::computed_value::T,
        pub color: longhands::color::computed_value::T,
        pub text_decoration: longhands::text_decoration::computed_value::T,
        pub font_size: longhands::font_size::computed_value::T,
        pub root_font_size: longhands::font_size::computed_value::T,
        pub display: longhands::display::computed_value::T,
        pub positioned: bool,
        pub floated: bool,
        pub border_top_present: bool,
        pub border_right_present: bool,
        pub border_bottom_present: bool,
        pub border_left_present: bool,
        pub is_root_element: bool,
        // TODO, as needed: viewport size, etc.
    }

    #[allow(non_snake_case)]
    #[inline]
    pub fn compute_CSSColor(value: specified::CSSColor, _context: &Context) -> CSSColor {
        value.parsed
    }

    #[allow(non_snake_case)]
    #[inline]
    pub fn compute_BorderStyle(value: BorderStyle, _context: &Context) -> BorderStyle {
        value
    }

    #[allow(non_snake_case)]
    #[inline]
    pub fn compute_Au(value: specified::Length, context: &Context) -> Au {
        compute_Au_with_font_size(value, context.font_size, context.root_font_size)
    }

    /// A special version of `compute_Au` used for `font-size`.
    #[allow(non_snake_case)]
    #[inline]
    pub fn compute_Au_with_font_size(value: specified::Length, reference_font_size: Au, root_font_size: Au) -> Au {
        match value {
            specified::Length::Au(value) => value,
            specified::Length::Em(value) => reference_font_size.scale_by(value),
            specified::Length::Ex(value) => {
                let x_height = 0.5;  // TODO: find that from the font
                reference_font_size.scale_by(value * x_height)
            },
            specified::Length::Rem(value) => root_font_size.scale_by(value),
            specified::Length::ServoCharacterWidth(value) => {
                // This applies the *converting a character width to pixels* algorithm as specified
                // in HTML5 § 14.5.4.
                //
                // TODO(pcwalton): Find these from the font.
                let average_advance = reference_font_size.scale_by(0.5);
                let max_advance = reference_font_size;
                average_advance.scale_by(value as CSSFloat - 1.0) + max_advance
            }
        }
    }

    #[derive(PartialEq, Clone, Copy)]
    pub enum LengthOrPercentage {
        Length(Au),
        Percentage(CSSFloat),
    }
    impl fmt::Show for LengthOrPercentage {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                &LengthOrPercentage::Length(length) => write!(f, "{:?}", length),
                &LengthOrPercentage::Percentage(percentage) => write!(f, "{}%", percentage * 100.),
            }
        }
    }

    #[allow(non_snake_case)]
    pub fn compute_LengthOrPercentage(value: specified::LengthOrPercentage, context: &Context)
                                   -> LengthOrPercentage {
        match value {
            specified::LengthOrPercentage::Length(value) =>
                LengthOrPercentage::Length(compute_Au(value, context)),
            specified::LengthOrPercentage::Percentage(value) =>
                LengthOrPercentage::Percentage(value),
        }
    }

    #[derive(PartialEq, Clone, Copy)]
    pub enum LengthOrPercentageOrAuto {
        Length(Au),
        Percentage(CSSFloat),
        Auto,
    }
    impl fmt::Show for LengthOrPercentageOrAuto {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                &LengthOrPercentageOrAuto::Length(length) => write!(f, "{:?}", length),
                &LengthOrPercentageOrAuto::Percentage(percentage) => write!(f, "{}%", percentage * 100.),
                &LengthOrPercentageOrAuto::Auto => write!(f, "auto"),
            }
        }
    }
    #[allow(non_snake_case)]
    pub fn compute_LengthOrPercentageOrAuto(value: specified::LengthOrPercentageOrAuto,
                                            context: &Context) -> LengthOrPercentageOrAuto {
        match value {
            specified::LengthOrPercentageOrAuto::Length(value) =>
                LengthOrPercentageOrAuto::Length(compute_Au(value, context)),
            specified::LengthOrPercentageOrAuto::Percentage(value) =>
                LengthOrPercentageOrAuto::Percentage(value),
            specified::LengthOrPercentageOrAuto::Auto =>
                LengthOrPercentageOrAuto::Auto,
        }
    }

    #[derive(PartialEq, Clone, Copy)]
    pub enum LengthOrPercentageOrNone {
        Length(Au),
        Percentage(CSSFloat),
        None,
    }
    impl fmt::Show for LengthOrPercentageOrNone {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                &LengthOrPercentageOrNone::Length(length) => write!(f, "{:?}", length),
                &LengthOrPercentageOrNone::Percentage(percentage) => write!(f, "{}%", percentage * 100.),
                &LengthOrPercentageOrNone::None => write!(f, "none"),
            }
        }
    }
    #[allow(non_snake_case)]
    pub fn compute_LengthOrPercentageOrNone(value: specified::LengthOrPercentageOrNone,
                                            context: &Context) -> LengthOrPercentageOrNone {
        match value {
            specified::LengthOrPercentageOrNone::Length(value) =>
                LengthOrPercentageOrNone::Length(compute_Au(value, context)),
            specified::LengthOrPercentageOrNone::Percentage(value) =>
                LengthOrPercentageOrNone::Percentage(value),
            specified::LengthOrPercentageOrNone::None =>
                LengthOrPercentageOrNone::None,
        }
    }

    /// Computed values for an image according to CSS-IMAGES.
    #[derive(Clone, PartialEq)]
    pub enum Image {
        Url(Url),
        LinearGradient(LinearGradient),
    }

    impl fmt::Show for Image {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                &Image::Url(ref url) => write!(f, "url(\"{}\")", url),
                &Image::LinearGradient(ref grad) => write!(f, "linear-gradient({:?})", grad),
            }
        }
    }

    /// Computed values for a CSS linear gradient.
    #[derive(Clone, PartialEq)]
    pub struct LinearGradient {
        /// The angle or corner of the gradient.
        pub angle_or_corner: AngleOrCorner,

        /// The color stops.
        pub stops: Vec<ColorStop>,
    }

    impl fmt::Show for LinearGradient {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let _ = write!(f, "{:?}", self.angle_or_corner);
            for stop in self.stops.iter() {
                let _ = write!(f, ", {:?}", stop);
            }
            Ok(())
        }
    }

    /// Computed values for one color stop in a linear gradient.
    #[derive(Clone, PartialEq, Copy)]
    pub struct ColorStop {
        /// The color of this stop.
        pub color: CSSColor,

        /// The position of this stop. If not specified, this stop is placed halfway between the
        /// point that precedes it and the point that follows it per CSS-IMAGES § 3.4.
        pub position: Option<LengthOrPercentage>,
    }

    impl fmt::Show for ColorStop {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let _ = write!(f, "{:?}", self.color);
            self.position.map(|pos| {
                let _ = write!(f, " {:?}", pos);
            });
            Ok(())
        }
    }

    impl LinearGradient {
        pub fn compute(value: specified::LinearGradient, context: &Context) -> LinearGradient {
            let specified::LinearGradient {
                angle_or_corner,
                stops
            } = value;
            LinearGradient {
                angle_or_corner: angle_or_corner,
                stops: stops.into_iter().map(|stop| {
                    ColorStop {
                        color: stop.color.parsed,
                        position: match stop.position {
                            None => None,
                            Some(value) => Some(compute_LengthOrPercentage(value, context)),
                        },
                    }
                }).collect()
            }
        }
    }

    #[allow(non_snake_case)]
    #[inline]
    pub fn compute_Length(value: specified::Length, context: &Context) -> Au {
        compute_Au(value, context)
    }

    pub type Length = Au;
}
