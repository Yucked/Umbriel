use crate::{Fragment, QueryChain, Queryable, Render, Renderer, ValueLike};

/// A code block. Surrounds the code with curly braces.
/// # Examples
/// ```
/// use umbriel::{*, statements::*, functions::*};
///
/// let sales = Table::new("sales");
/// let metrics = Table::new("metrics");
/// let quantity = Field::new("quantity");
/// let average_sales = Field::new("average_sales");
///
/// let ref sales = let_("sales").equal_to(select_value(quantity).from(sales));
/// let ref total = let_("total").equal_to(math::sum!(sales));
/// let ref count = let_("count").equal_to(count!(sales));
///
/// let returned = return_(bracket(total.divide(count)));
/// let code_block = block(chain(sales).chain(total).chain(count).chain(returned));
///
/// let def = define_field(average_sales).on_table(metrics).value(code_block);
///
/// assert_eq!(
///     def.to_debug_sql(),
///     "DEFINE FIELD average_sales ON TABLE metrics VALUE {\n\
///         LET $sales = (SELECT VALUE quantity FROM sales);\n\n\
///         LET $total = math::sum($sales);\n\n\
///         LET $count = count($sales);\n\n\
///         RETURN ($total / $count);\n\
///         };"
/// );
pub fn block(code: QueryChain) -> Block {
    Block(code)
}

/// A code block. Surrounds the code with curly braces.
#[derive(Debug, Clone, PartialEq)]
pub struct Block(QueryChain);

impl Render for Block {
    fn render(&self, out: &mut Renderer) {
        out.push_str("{\n").push(&self.0).push_str("\n}");
    }
}

impl From<QueryChain> for Block {
    fn from(code: QueryChain) -> Self {
        Self(code)
    }
}

impl From<Block> for ValueLike {
    fn from(block: Block) -> Self {
        ValueLike::new(block)
    }
}

impl Queryable for Block {
    fn into_fragment(self) -> Fragment {
        Fragment::new(self)
    }
}
