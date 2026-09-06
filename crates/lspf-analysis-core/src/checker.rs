use std::sync::OnceLock;

use regex::bytes::Regex;

use crate::*;

static RE: OnceLock<Regex> = OnceLock::new();

macro_rules! check_if_func {
    ($parser: ident, $language: ident, $node: ident) => {
        $node.count_specific_ancestors::<$parser>(
            |node| {
                matches!(
                    $language::from(node.kind_id()),
                    $language::VariableDeclarator
                        | $language::AssignmentExpression
                        | $language::LabeledStatement
                        | $language::Pair
                )
            },
            |node| {
                matches!(
                    $language::from(node.kind_id()),
                    $language::StatementBlock
                        | $language::ReturnStatement
                        | $language::NewExpression
                        | $language::Arguments
                )
            },
        ) > 0
            || $node.is_child($language::Identifier as u16)
    };
}

macro_rules! check_if_arrow_func {
    ($parser: ident, $language: ident, $node: ident) => {
        $node.count_specific_ancestors::<$parser>(
            |node| {
                matches!(
                    $language::from(node.kind_id()),
                    $language::VariableDeclarator
                        | $language::AssignmentExpression
                        | $language::LabeledStatement
                )
            },
            |node| {
                matches!(
                    $language::from(node.kind_id()),
                    $language::StatementBlock
                        | $language::ReturnStatement
                        | $language::NewExpression
                        | $language::CallExpression
                )
            },
        ) > 0
            || $node.has_sibling($language::PropertyIdentifier as u16)
    };
}

macro_rules! is_js_func {
    ($parser: ident, $language: ident, $node: ident) => {
        match $language::from($node.kind_id()) {
            $language::FunctionDeclaration | $language::MethodDefinition => true,
            $language::FunctionExpression => check_if_func!($parser, $language, $node),
            $language::ArrowFunction => check_if_arrow_func!($parser, $language, $node),
            _ => false,
        }
    };
}

macro_rules! is_js_closure {
    ($parser: ident, $language: ident, $node: ident) => {
        match $language::from($node.kind_id()) {
            $language::GeneratorFunction | $language::GeneratorFunctionDeclaration => true,
            $language::FunctionExpression => !check_if_func!($parser, $language, $node),
            $language::ArrowFunction => !check_if_arrow_func!($parser, $language, $node),
            _ => false,
        }
    };
}

macro_rules! is_js_func_and_closure_checker {
    ($parser: ident, $language: ident) => {
        #[inline(always)]
        fn is_func(node: &Node) -> bool {
            is_js_func!($parser, $language, node)
        }

        #[inline(always)]
        fn is_closure(node: &Node) -> bool {
            is_js_closure!($parser, $language, node)
        }
    };
}

pub trait Checker {
    fn is_comment(_: &Node) -> bool;
    fn is_useful_comment(_: &Node, _: &[u8]) -> bool;
    fn is_func_space(_: &Node) -> bool;
    fn is_func(_: &Node) -> bool;
    fn is_closure(_: &Node) -> bool;
    fn is_call(_: &Node) -> bool;
    fn is_non_arg(_: &Node) -> bool;
    fn is_string(_: &Node) -> bool;
    fn is_else_if(_: &Node) -> bool;
    fn is_primitive(_id: u16) -> bool;

    fn is_error(node: &Node) -> bool {
        node.has_error()
    }
}

impl Checker for PythonCode {
    fn is_comment(node: &Node) -> bool {
        node.kind_id() == Python::Comment as u16
    }

    fn is_useful_comment(node: &Node, code: &[u8]) -> bool {
        // comment containing coding info are useful
        node.start_row() <= 1
            && RE
                .get_or_init(|| {
                    Regex::new(r"^[ \t\f]*#.*?coding[:=][ \t]*([-_.a-zA-Z0-9]+)").unwrap()
                })
                .is_match(&code[node.start_byte()..node.end_byte()])
    }

    fn is_func_space(node: &Node) -> bool {
        matches!(
            Python::from(node.kind_id()),
            Python::Module | Python::FunctionDefinition | Python::ClassDefinition
        )
    }

    fn is_func(node: &Node) -> bool {
        node.kind_id() == Python::FunctionDefinition as u16
    }

    fn is_closure(node: &Node) -> bool {
        node.kind_id() == Python::Lambda as u16
    }

    fn is_call(node: &Node) -> bool {
        node.kind_id() == Python::Call as u16
    }

    fn is_non_arg(node: &Node) -> bool {
        matches!(
            Python::from(node.kind_id()),
            Python::LPAREN | Python::COMMA | Python::RPAREN
        )
    }

    fn is_string(node: &Node) -> bool {
        matches!(
            Python::from(node.kind_id()),
            Python::String | Python::ConcatenatedString
        )
    }

    fn is_else_if(_: &Node) -> bool {
        false
    }

    fn is_primitive(_id: u16) -> bool {
        false
    }
}

impl Checker for JavascriptCode {
    fn is_comment(node: &Node) -> bool {
        node.kind_id() == Javascript::Comment as u16
    }

    fn is_useful_comment(_: &Node, _: &[u8]) -> bool {
        false
    }

    fn is_func_space(node: &Node) -> bool {
        matches!(
            Javascript::from(node.kind_id()),
            Javascript::Program
                | Javascript::FunctionExpression
                | Javascript::Class
                | Javascript::GeneratorFunction
                | Javascript::FunctionDeclaration
                | Javascript::MethodDefinition
                | Javascript::GeneratorFunctionDeclaration
                | Javascript::ClassDeclaration
                | Javascript::ArrowFunction
        )
    }

    is_js_func_and_closure_checker!(JavascriptParser, Javascript);

    fn is_call(node: &Node) -> bool {
        node.kind_id() == Javascript::CallExpression as u16
    }

    fn is_non_arg(node: &Node) -> bool {
        matches!(
            Javascript::from(node.kind_id()),
            Javascript::LPAREN | Javascript::COMMA | Javascript::RPAREN
        )
    }

    fn is_string(node: &Node) -> bool {
        matches!(
            Javascript::from(node.kind_id()),
            Javascript::String | Javascript::TemplateString
        )
    }

    #[inline(always)]
    fn is_else_if(node: &Node) -> bool {
        if node.kind_id() != Javascript::IfStatement as u16 {
            return false;
        }
        if let Some(parent) = node.parent() {
            return parent.kind_id() == Javascript::IfStatement as u16;
        }
        false
    }

    fn is_primitive(_id: u16) -> bool {
        false
    }
}

impl Checker for TypescriptCode {
    fn is_comment(node: &Node) -> bool {
        node.kind_id() == Typescript::Comment as u16
    }

    fn is_useful_comment(_: &Node, _: &[u8]) -> bool {
        false
    }

    fn is_func_space(node: &Node) -> bool {
        matches!(
            Typescript::from(node.kind_id()),
            Typescript::Program
                | Typescript::FunctionExpression
                | Typescript::Class
                | Typescript::GeneratorFunction
                | Typescript::FunctionDeclaration
                | Typescript::MethodDefinition
                | Typescript::GeneratorFunctionDeclaration
                | Typescript::ClassDeclaration
                | Typescript::InterfaceDeclaration
                | Typescript::ArrowFunction
        )
    }

    is_js_func_and_closure_checker!(TypescriptParser, Typescript);

    fn is_call(node: &Node) -> bool {
        node.kind_id() == Typescript::CallExpression as u16
    }

    fn is_non_arg(node: &Node) -> bool {
        matches!(
            Typescript::from(node.kind_id()),
            Typescript::LPAREN | Typescript::COMMA | Typescript::RPAREN
        )
    }

    fn is_string(node: &Node) -> bool {
        matches!(
            Typescript::from(node.kind_id()),
            Typescript::String | Typescript::TemplateString
        )
    }

    #[inline(always)]
    fn is_else_if(node: &Node) -> bool {
        if node.kind_id() != Typescript::IfStatement as u16 {
            return false;
        }
        if let Some(parent) = node.parent() {
            return parent.kind_id() == Typescript::ElseClause as u16;
        }
        false
    }

    #[inline(always)]
    fn is_primitive(id: u16) -> bool {
        id == Typescript::PredefinedType as u16
    }
}

impl Checker for TsxCode {
    fn is_comment(node: &Node) -> bool {
        node.kind_id() == Tsx::Comment as u16
    }

    fn is_useful_comment(_: &Node, _: &[u8]) -> bool {
        false
    }

    fn is_func_space(node: &Node) -> bool {
        matches!(
            Tsx::from(node.kind_id()),
            Tsx::Program
                | Tsx::FunctionExpression
                | Tsx::Class
                | Tsx::GeneratorFunction
                | Tsx::FunctionDeclaration
                | Tsx::MethodDefinition
                | Tsx::GeneratorFunctionDeclaration
                | Tsx::ClassDeclaration
                | Tsx::InterfaceDeclaration
                | Tsx::ArrowFunction
        )
    }

    is_js_func_and_closure_checker!(TsxParser, Tsx);

    fn is_call(node: &Node) -> bool {
        node.kind_id() == Tsx::CallExpression as u16
    }

    fn is_non_arg(node: &Node) -> bool {
        matches!(
            Tsx::from(node.kind_id()),
            Tsx::LPAREN | Tsx::COMMA | Tsx::RPAREN
        )
    }

    fn is_string(node: &Node) -> bool {
        matches!(Tsx::from(node.kind_id()), Tsx::String | Tsx::TemplateString)
    }

    fn is_else_if(node: &Node) -> bool {
        if node.kind_id() != Tsx::IfStatement as u16 {
            return false;
        }
        if let Some(parent) = node.parent() {
            return parent.kind_id() == Tsx::IfStatement as u16;
        }
        false
    }

    #[inline(always)]
    fn is_primitive(id: u16) -> bool {
        id == Tsx::PredefinedType as u16
    }
}

impl Checker for JavaCode {
    fn is_comment(node: &Node) -> bool {
        matches!(
            Java::from(node.kind_id()),
            Java::LineComment | Java::BlockComment
        )
    }

    fn is_useful_comment(_: &Node, _: &[u8]) -> bool {
        false
    }

    fn is_func_space(node: &Node) -> bool {
        matches!(
            Java::from(node.kind_id()),
            Java::Program | Java::ClassDeclaration | Java::InterfaceDeclaration
        )
    }

    fn is_func(node: &Node) -> bool {
        matches!(
            Java::from(node.kind_id()),
            Java::MethodDeclaration | Java::ConstructorDeclaration
        )
    }

    fn is_closure(node: &Node) -> bool {
        node.kind_id() == Java::LambdaExpression as u16
    }

    fn is_call(node: &Node) -> bool {
        node.kind_id() == Java::MethodInvocation as u16
    }

    // Upstream returns `false` here, which makes `NArgs` count the
    // parentheses and commas of a `formal_parameters` node as arguments —
    // `f(int a, int b)` reports five. The interface pillar of the health
    // score reads that number directly, so the separators are excluded the
    // same way every other language excludes them.
    fn is_non_arg(node: &Node) -> bool {
        matches!(
            Java::from(node.kind_id()),
            Java::LPAREN | Java::COMMA | Java::RPAREN
        )
    }

    fn is_string(node: &Node) -> bool {
        node.kind_id() == Java::StringLiteral as u16
    }

    fn is_else_if(_: &Node) -> bool {
        false
    }

    fn is_primitive(_id: u16) -> bool {
        false
    }
}

impl Checker for RustCode {
    fn is_comment(node: &Node) -> bool {
        matches!(
            Rust::from(node.kind_id()),
            Rust::LineComment | Rust::BlockComment
        )
    }

    fn is_useful_comment(node: &Node, code: &[u8]) -> bool {
        if let Some(parent) = node.parent()
            && parent.kind_id() == Rust::TokenTree as u16
        {
            // A comment could be a macro token
            return true;
        }
        let code = &code[node.start_byte()..node.end_byte()];
        code.starts_with(b"/// cbindgen:")
    }

    fn is_func_space(node: &Node) -> bool {
        matches!(
            Rust::from(node.kind_id()),
            Rust::SourceFile
                | Rust::FunctionItem
                | Rust::ImplItem
                | Rust::TraitItem
                | Rust::ClosureExpression
        )
    }

    fn is_func(node: &Node) -> bool {
        node.kind_id() == Rust::FunctionItem as u16
    }

    fn is_closure(node: &Node) -> bool {
        node.kind_id() == Rust::ClosureExpression as u16
    }

    fn is_call(node: &Node) -> bool {
        node.kind_id() == Rust::CallExpression as u16
    }

    fn is_non_arg(node: &Node) -> bool {
        matches!(
            Rust::from(node.kind_id()),
            Rust::LPAREN | Rust::COMMA | Rust::RPAREN | Rust::PIPE | Rust::AttributeItem
        )
    }

    fn is_string(node: &Node) -> bool {
        matches!(
            Rust::from(node.kind_id()),
            Rust::StringLiteral | Rust::RawStringLiteral
        )
    }

    #[inline(always)]
    fn is_else_if(node: &Node) -> bool {
        if node.kind_id() != Rust::IfExpression as u16 {
            return false;
        }
        if let Some(parent) = node.parent() {
            return parent.kind_id() == Rust::ElseClause as u16;
        }
        false
    }

    #[inline(always)]
    fn is_primitive(id: u16) -> bool {
        id == Rust::PrimitiveType as u16
    }
}

impl Checker for CppCode {
    fn is_comment(node: &Node) -> bool {
        node.kind_id() == Cpp::Comment as u16
    }

    fn is_useful_comment(_: &Node, _: &[u8]) -> bool {
        false
    }

    fn is_func_space(node: &Node) -> bool {
        matches!(
            Cpp::from(node.kind_id()),
            Cpp::LambdaExpression
                | Cpp::UnionSpecifier
                | Cpp::TranslationUnit
                | Cpp::FunctionDefinition
                | Cpp::FunctionDefinition2
                | Cpp::FunctionDefinition3
                | Cpp::FunctionDefinition4
                | Cpp::StructSpecifier
                | Cpp::ClassSpecifier
                | Cpp::NamespaceDefinition
        )
    }

    fn is_func(node: &Node) -> bool {
        matches!(
            Cpp::from(node.kind_id()),
            Cpp::FunctionDefinition
                | Cpp::FunctionDefinition2
                | Cpp::FunctionDefinition3
                | Cpp::FunctionDefinition4
        )
    }

    fn is_closure(node: &Node) -> bool {
        node.kind_id() == Cpp::LambdaExpression as u16
    }

    fn is_call(node: &Node) -> bool {
        matches!(
            Cpp::from(node.kind_id()),
            Cpp::CallExpression | Cpp::CallExpression2
        )
    }

    fn is_non_arg(node: &Node) -> bool {
        matches!(
            Cpp::from(node.kind_id()),
            Cpp::LPAREN | Cpp::LPAREN2 | Cpp::COMMA | Cpp::RPAREN | Cpp::Comment
        )
    }

    fn is_string(node: &Node) -> bool {
        matches!(
            Cpp::from(node.kind_id()),
            Cpp::StringLiteral | Cpp::ConcatenatedString | Cpp::RawStringLiteral
        )
    }

    fn is_else_if(node: &Node) -> bool {
        if node.kind_id() != Cpp::IfStatement as u16 {
            return false;
        }
        if let Some(parent) = node.parent() {
            return parent.kind_id() == Cpp::ElseClause as u16;
        }
        false
    }

    #[inline(always)]
    fn is_primitive(id: u16) -> bool {
        id == Cpp::PrimitiveType as u16
    }
}
