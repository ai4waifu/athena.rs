//! Schema、字段与缺失语义。

use crate::TableError;

/// 逻辑数据类型（Athena 自有，非 Arrow DataType 真相源）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogicalType {
    /// 布尔。
    Boolean,
    /// 无符号整数位宽。
    UInt(u8),
    /// 有符号整数位宽。
    Int(u8),
    /// 浮点位宽。
    Float(u8),
    /// 精确整数（由 numeric 层解释）。
    ExactInteger,
    /// 有理数。
    ExactRational,
    /// UTF-8 字符串。
    Utf8,
    /// 二进制。
    Binary,
    /// 嵌套列表。
    List(Box<LogicalType>),
    /// 结构体字段。
    Struct(Vec<Field>),
    /// 扩展 / 无法映射的域类型名。
    Extension(&'static str),
}

/// 列字段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    name: String,
    data_type: LogicalType,
    nullable: bool,
}

impl Field {
    /// 创建字段。
    pub fn new(name: impl Into<String>, data_type: LogicalType, nullable: bool) -> Self {
        Self { name: name.into(), data_type, nullable }
    }

    /// 列名。
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 逻辑类型。
    pub const fn data_type(&self) -> &LogicalType {
        &self.data_type
    }

    /// 是否允许 null。
    pub const fn nullable(&self) -> bool {
        self.nullable
    }
}

/// 表 schema。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    fields: Vec<Field>,
}

impl Schema {
    /// 创建 schema；拒绝重复列名。
    pub fn new(fields: impl Into<Vec<Field>>) -> Result<Self, TableError> {
        let fields = fields.into();
        for i in 0..fields.len() {
            for j in (i + 1)..fields.len() {
                if fields[i].name == fields[j].name {
                    return Err(TableError::DuplicateField(fields[i].name.clone()));
                }
            }
        }
        Ok(Self { fields })
    }

    /// 字段列表。
    pub fn fields(&self) -> &[Field] {
        &self.fields
    }

    /// 列数。
    pub fn width(&self) -> usize {
        self.fields.len()
    }

    /// 按名查找字段。
    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.name == name)
    }
}

/// 缺失 / 未知 / 掩码等语义（不得压成同一 sentinel）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Absence {
    /// Arrow/DataFrame 式字段无值。
    Null,
    /// 带原因的数据缺失。
    Missing(&'static str),
    /// 数学事实尚不确定。
    Unknown,
    /// 数学运算本身不定。
    Indeterminate,
    /// IEEE 浮点 NaN（仅浮点域）。
    NaN,
    /// 数组计算临时排除。
    Masked,
    /// schema 存在但该记录不适用。
    NotApplicable,
}
