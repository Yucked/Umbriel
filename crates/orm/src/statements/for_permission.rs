use crate::{Conditional, CrudType, Filter, Fragment, Queryable, Raw, Render, Renderer};

/// The CRUD operations a permission applies to.
#[derive(Clone, Debug)]
pub enum ForArgs {
    /// One operation
    ForOption(CrudType),
    /// Several operations
    ForOptions(Vec<CrudType>),
}

impl From<CrudType> for ForArgs {
    fn from(value: CrudType) -> Self {
        Self::ForOption(value)
    }
}

impl From<Vec<CrudType>> for ForArgs {
    fn from(value: Vec<CrudType>) -> Self {
        Self::ForOptions(value)
    }
}

impl From<ForArgs> for Vec<CrudType> {
    fn from(value: ForArgs) -> Self {
        match value {
            ForArgs::ForOption(one) => vec![one],
            ForArgs::ForOptions(many) => many,
        }
    }
}

impl<const N: usize> From<&[CrudType; N]> for ForArgs {
    fn from(value: &[CrudType; N]) -> Self {
        Self::ForOptions(value.to_vec())
    }
}

impl<const N: usize> From<[CrudType; N]> for ForArgs {
    fn from(value: [CrudType; N]) -> Self {
        Self::ForOptions(value.to_vec())
    }
}

/// A `FOR` permission awaiting its condition.
pub struct ForStart(Vec<CrudType>);

impl ForStart {
    /// Sets the condition under which the operations are permitted.
    pub fn where_(self, condition: impl Conditional) -> ForPermission {
        ForPermission {
            crud_types: self.0,
            condition: Filter::new(condition),
        }
    }
}

/// For statement is typically used within DEFINE TABLE and DEFINE FIELD for create
/// more granular permissions.
///
/// Examples:
///
/// ```rust
/// use umbriel::{*, statements::for_permission};
/// use CrudType::*;
///
/// # let name = Field::new("name");
/// # let country = Field::new("country");
///
///  // You can create for a single crud operation
/// let statement = for_permission(Create).where_(name.is("Alice"));
///
///  // You can also create for a  list of crud operations
/// let statement = for_permission(&[Create, Delete, Select, Update]).where_(country.is("Canada"));
///
/// assert!(!statement.to_debug_sql().is_empty());
/// ```
pub fn for_permission(for_crud_types: impl Into<ForArgs>) -> ForStart {
    ForStart(for_crud_types.into().into())
}

/// Builder struct for a `FOR` statement which is typeically used in DEFINE TABLE and DEFINE FIELD
/// statements for setting more granular permissions
#[derive(Clone, Debug)]
pub struct ForPermission {
    crud_types: Vec<CrudType>,
    condition: Filter,
}

impl Render for ForPermission {
    fn render(&self, out: &mut Renderer) {
        out.push_str("FOR");
        if !self.crud_types.is_empty() {
            out.push_str(" ").push(&self.crud_types);
        }
        out.push_str("\n\tWHERE ").push(&self.condition);
    }
}

impl Queryable for ForPermission {
    fn into_fragment(self) -> Fragment {
        Fragment::new(self)
    }
}

/// Permission types which can be a a single For statement
/// list of `for` statements
#[derive(Clone, Debug)]
pub enum Permissions {
    /// Single `for` statement
    For(ForPermission),
    /// List of `for` statements
    Fors(Vec<ForPermission>),
    /// Single Raw statement
    RawStatement(Raw),
    /// List of Raw statements
    RawStatementList(Vec<Raw>),
}

/// Permissions render one per line.
impl Render for Permissions {
    fn render(&self, out: &mut Renderer) {
        match self {
            Permissions::For(one) => one.render(out),
            Permissions::Fors(many) => {
                out.push_joined(many, "\n");
            }
            Permissions::RawStatement(raw) => raw.render(out),
            Permissions::RawStatementList(raws) => {
                out.push_joined(raws, "\n");
            }
        }
    }
}

/// The `PERMISSIONS` clause shared by definitions.
#[derive(Clone, Debug, Default)]
pub(crate) struct PermissionsClause {
    none: bool,
    full: bool,
    fors: Vec<Permissions>,
}

impl PermissionsClause {
    pub(crate) fn none(&mut self) {
        self.none = true;
    }

    pub(crate) fn full(&mut self) {
        self.full = true;
    }

    /// Additional permissions accumulate.
    pub(crate) fn push(&mut self, permissions: impl Into<Permissions>) {
        self.fors.push(permissions.into());
    }
}

/// Renders ` PERMISSIONS NONE`, ` PERMISSIONS FULL`, or the `FOR` permissions one per line;
/// nothing when unset.
impl Render for PermissionsClause {
    fn render(&self, out: &mut Renderer) {
        if self.none {
            out.push_str(" PERMISSIONS NONE");
        } else if self.full {
            out.push_str(" PERMISSIONS FULL");
        } else if !self.fors.is_empty() {
            out.push_str("\nPERMISSIONS\n")
                .push_joined(&self.fors, "\n");
        }
    }
}

impl From<ForPermission> for Permissions {
    fn from(value: ForPermission) -> Self {
        Self::For(value)
    }
}

impl From<Vec<ForPermission>> for Permissions {
    fn from(value: Vec<ForPermission>) -> Self {
        Self::Fors(value)
    }
}

impl<const N: usize> From<&[ForPermission; N]> for Permissions {
    fn from(value: &[ForPermission; N]) -> Self {
        Self::Fors(value.to_vec())
    }
}

impl<const N: usize> From<[ForPermission; N]> for Permissions {
    fn from(value: [ForPermission; N]) -> Self {
        Self::Fors(value.to_vec())
    }
}

impl From<Raw> for Permissions {
    fn from(value: Raw) -> Self {
        Self::RawStatement(value)
    }
}

impl From<Vec<Raw>> for Permissions {
    fn from(value: Vec<Raw>) -> Self {
        Self::RawStatementList(value)
    }
}

impl From<&Vec<Raw>> for Permissions {
    fn from(value: &Vec<Raw>) -> Self {
        Self::RawStatementList(value.to_vec())
    }
}

impl<const N: usize> From<&[Raw; N]> for Permissions {
    fn from(value: &[Raw; N]) -> Self {
        Self::RawStatementList(value.to_vec())
    }
}

impl<const N: usize> From<[Raw; N]> for Permissions {
    fn from(value: [Raw; N]) -> Self {
        Self::RawStatementList(value.to_vec())
    }
}

/// A single permission without `FOR`: `PERMISSIONS NONE | FULL | WHERE @condition`, as params and
/// functions take it.
#[derive(Clone, Debug)]
pub(crate) enum PermissionValue {
    None,
    Full,
    Where(Filter),
}

impl Render for PermissionValue {
    fn render(&self, out: &mut Renderer) {
        match self {
            Self::None => {
                out.push_str(" PERMISSIONS NONE");
            }
            Self::Full => {
                out.push_str(" PERMISSIONS FULL");
            }
            Self::Where(condition) => {
                out.push_str(" PERMISSIONS WHERE ").push(condition);
            }
        }
    }
}
