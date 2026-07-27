use std::fmt;
use std::str::FromStr;

/// Primitive data types in the DQIR intermediate representation.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Type {
    /// 1-bit integer / boolean condition.
    I1,
    /// 8-bit integer.
    I8,
    /// 16-bit integer.
    I16,
    /// 32-bit integer.
    I32,
    /// 64-bit integer.
    I64,
    /// 32-bit IEEE 754 floating-point number.
    F32,
    /// 64-bit IEEE 754 floating-point number.
    F64,
    /// Memory pointer (assumed 64-bit address space on target).
    Ptr,
    /// Void type (no return value or empty value).
    Void,
}

impl Type {
    /// Returns true if this type is an integer type (I1..I64).
    pub fn is_int(&self) -> bool {
        matches!(self, Type::I1 | Type::I8 | Type::I16 | Type::I32 | Type::I64)
    }

    /// Returns true if this type is a floating-point type (F32, F64).
    pub fn is_float(&self) -> bool {
        matches!(self, Type::F32 | Type::F64)
    }

    /// Returns true if this is the void type.
    pub fn is_void(&self) -> bool {
        matches!(self, Type::Void)
    }

    /// Returns the bit width of the type, or None for Void.
    pub fn bit_width(&self) -> Option<u32> {
        match self {
            Type::I1 => Some(1),
            Type::I8 => Some(8),
            Type::I16 => Some(16),
            Type::I32 | Type::F32 => Some(32),
            Type::I64 | Type::F64 | Type::Ptr => Some(64),
            Type::Void => None,
        }
    }

    /// Returns the byte size of the type (rounding up I1 to 1 byte), or None for Void.
    pub fn byte_size(&self) -> Option<u32> {
        match self {
            Type::I1 | Type::I8 => Some(1),
            Type::I16 => Some(2),
            Type::I32 | Type::F32 => Some(4),
            Type::I64 | Type::F64 | Type::Ptr => Some(8),
            Type::Void => None,
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Type::I1 => "i1",
            Type::I8 => "i8",
            Type::I16 => "i16",
            Type::I32 => "i32",
            Type::I64 => "i64",
            Type::F32 => "f32",
            Type::F64 => "f64",
            Type::Ptr => "ptr",
            Type::Void => "void",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for Type {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "i1" => Ok(Type::I1),
            "i8" => Ok(Type::I8),
            "i16" => Ok(Type::I16),
            "i32" => Ok(Type::I32),
            "i64" => Ok(Type::I64),
            "f32" => Ok(Type::F32),
            "f64" => Ok(Type::F64),
            "ptr" => Ok(Type::Ptr),
            "void" => Ok(Type::Void),
            _ => Err(format!("Unknown IR type: '{}'", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_display_and_parse() {
        let types = [
            Type::I1,
            Type::I8,
            Type::I16,
            Type::I32,
            Type::I64,
            Type::F32,
            Type::F64,
            Type::Ptr,
            Type::Void,
        ];
        for t in types {
            let s = t.to_string();
            let parsed = Type::from_str(&s).expect("should parse successfully");
            assert_eq!(t, parsed);
        }
    }
}
