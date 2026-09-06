use crate::metrics::halstead::HalsteadType;

use crate::spaces::SpaceKind;

use crate::*;

macro_rules! get_operator {
    ($language:ident) => {
        #[inline(always)]
        fn get_operator_id_as_str(id: u16) -> &'static str {
            let typ = id.into();
            match typ {
                $language::LPAREN => "()",
                $language::LBRACK => "[]",
                $language::LBRACE => "{}",
                _ => typ.into(),
            }
        }
    };
}

pub trait Getter {
    fn get_func_name<'a>(node: &Node, code: &'a [u8]) -> Option<&'a str> {
        Self::get_func_space_name(node, code)
    }

    fn get_func_space_name<'a>(node: &Node, code: &'a [u8]) -> Option<&'a str> {
        // we're in a function or in a class
        if let Some(name) = node.child_by_field_name("name") {
            let code = &code[name.start_byte()..name.end_byte()];
            std::str::from_utf8(code).ok()
        } else {
            Some("<anonymous>")
        }
    }

    fn get_space_kind(_node: &Node) -> SpaceKind {
        SpaceKind::Unknown
    }

    fn get_op_type(_node: &Node) -> HalsteadType {
        HalsteadType::Unknown
    }

    fn get_operator_id_as_str(_id: u16) -> &'static str {
        ""
    }
}

impl Getter for PythonCode {
    fn get_space_kind(node: &Node) -> SpaceKind {
        match Python::from(node.kind_id()) {
            Python::FunctionDefinition => SpaceKind::Function,
            Python::ClassDefinition => SpaceKind::Class,
            Python::Module => SpaceKind::Unit,
            _ => SpaceKind::Unknown,
        }
    }

    fn get_op_type(node: &Node) -> HalsteadType {
        match Python::from(node.kind_id()) {
            Python::Import
            | Python::DOT
            | Python::From
            | Python::COMMA
            | Python::As
            | Python::STAR
            | Python::GTGT
            | Python::Assert
            | Python::COLONEQ
            | Python::Return
            | Python::Def
            | Python::Del
            | Python::Raise
            | Python::Pass
            | Python::Break
            | Python::Continue
            | Python::If
            | Python::Elif
            | Python::Else
            | Python::Async
            | Python::For
            | Python::In
            | Python::While
            | Python::Try
            | Python::Except
            | Python::Finally
            | Python::With
            | Python::DASHGT
            | Python::EQ
            | Python::Global
            | Python::Exec
            | Python::AT
            | Python::Not
            | Python::And
            | Python::Or
            | Python::PLUS
            | Python::DASH
            | Python::SLASH
            | Python::PERCENT
            | Python::SLASHSLASH
            | Python::STARSTAR
            | Python::PIPE
            | Python::AMP
            | Python::CARET
            | Python::LTLT
            | Python::TILDE
            | Python::LT
            | Python::LTEQ
            | Python::EQEQ
            | Python::BANGEQ
            | Python::GTEQ
            | Python::GT
            | Python::LTGT
            | Python::Is
            | Python::PLUSEQ
            | Python::DASHEQ
            | Python::STAREQ
            | Python::SLASHEQ
            | Python::ATEQ
            | Python::SLASHSLASHEQ
            | Python::PERCENTEQ
            | Python::STARSTAREQ
            | Python::GTGTEQ
            | Python::LTLTEQ
            | Python::AMPEQ
            | Python::CARETEQ
            | Python::PIPEEQ
            | Python::Yield
            | Python::Await
            | Python::Await2
            | Python::Print => HalsteadType::Operator,
            Python::Identifier
            | Python::Integer
            | Python::Float
            | Python::True
            | Python::False
            | Python::None => HalsteadType::Operand,
            Python::String => {
                let mut operator = HalsteadType::Unknown;
                // check if we've a documentation string or a multiline comment
                if let Some(parent) = node.parent()
                    && (parent.kind_id() != Python::ExpressionStatement as u16
                        || parent.child_count() != 1)
                {
                    operator = HalsteadType::Operand;
                };
                operator
            }
            _ => HalsteadType::Unknown,
        }
    }

    fn get_operator_id_as_str(id: u16) -> &'static str {
        Into::<Python>::into(id).into()
    }
}

impl Getter for JavascriptCode {
    fn get_space_kind(node: &Node) -> SpaceKind {
        match Javascript::from(node.kind_id()) {
            Javascript::FunctionExpression
            | Javascript::MethodDefinition
            | Javascript::GeneratorFunction
            | Javascript::FunctionDeclaration
            | Javascript::GeneratorFunctionDeclaration
            | Javascript::ArrowFunction => SpaceKind::Function,
            Javascript::Class | Javascript::ClassDeclaration => SpaceKind::Class,
            Javascript::Program => SpaceKind::Unit,
            _ => SpaceKind::Unknown,
        }
    }

    fn get_func_space_name<'a>(node: &Node, code: &'a [u8]) -> Option<&'a str> {
        if let Some(name) = node.child_by_field_name("name") {
            let code = &code[name.start_byte()..name.end_byte()];
            std::str::from_utf8(code).ok()
        } else {
            // We can be in a pair: foo: function() {}
            // Or in a variable declaration: var aFun = function() {}
            if let Some(parent) = node.parent() {
                match Javascript::from(parent.kind_id()) {
                    Javascript::Pair => {
                        if let Some(name) = parent.child_by_field_name("key") {
                            let code = &code[name.start_byte()..name.end_byte()];
                            return std::str::from_utf8(code).ok();
                        }
                    }
                    Javascript::VariableDeclarator => {
                        if let Some(name) = parent.child_by_field_name("name") {
                            let code = &code[name.start_byte()..name.end_byte()];
                            return std::str::from_utf8(code).ok();
                        }
                    }
                    _ => {}
                }
            }
            Some("<anonymous>")
        }
    }

    fn get_op_type(node: &Node) -> HalsteadType {
        match Javascript::from(node.kind_id()) {
            Javascript::Export
            | Javascript::Import
            | Javascript::Import2
            | Javascript::Extends
            | Javascript::DOT
            | Javascript::From
            | Javascript::LPAREN
            | Javascript::COMMA
            | Javascript::As
            | Javascript::STAR
            | Javascript::GTGT
            | Javascript::GTGTGT
            | Javascript::COLON
            | Javascript::Return
            | Javascript::Delete
            | Javascript::Throw
            | Javascript::Break
            | Javascript::Continue
            | Javascript::If
            | Javascript::Else
            | Javascript::Switch
            | Javascript::Case
            | Javascript::Default
            | Javascript::Async
            | Javascript::For
            | Javascript::In
            | Javascript::Of
            | Javascript::While
            | Javascript::Try
            | Javascript::Catch
            | Javascript::Finally
            | Javascript::With
            | Javascript::EQ
            | Javascript::AT
            | Javascript::AMPAMP
            | Javascript::PIPEPIPE
            | Javascript::PLUS
            | Javascript::DASH
            | Javascript::DASHDASH
            | Javascript::PLUSPLUS
            | Javascript::SLASH
            | Javascript::PERCENT
            | Javascript::STARSTAR
            | Javascript::PIPE
            | Javascript::AMP
            | Javascript::LTLT
            | Javascript::TILDE
            | Javascript::LT
            | Javascript::LTEQ
            | Javascript::EQEQ
            | Javascript::BANGEQ
            | Javascript::GTEQ
            | Javascript::GT
            | Javascript::PLUSEQ
            | Javascript::BANG
            | Javascript::BANGEQEQ
            | Javascript::EQEQEQ
            | Javascript::DASHEQ
            | Javascript::STAREQ
            | Javascript::SLASHEQ
            | Javascript::PERCENTEQ
            | Javascript::STARSTAREQ
            | Javascript::GTGTEQ
            | Javascript::GTGTGTEQ
            | Javascript::LTLTEQ
            | Javascript::AMPEQ
            | Javascript::CARET
            | Javascript::CARETEQ
            | Javascript::PIPEEQ
            | Javascript::Yield
            | Javascript::LBRACK
            | Javascript::LBRACE
            | Javascript::Await
            | Javascript::QMARK
            | Javascript::QMARKQMARK
            | Javascript::New
            | Javascript::Let
            | Javascript::Var
            | Javascript::Const
            | Javascript::Function
            | Javascript::FunctionExpression
            | Javascript::SEMI => HalsteadType::Operator,
            Javascript::Identifier
            | Javascript::Identifier2
            | Javascript::MemberExpression
            | Javascript::MemberExpression2
            | Javascript::PropertyIdentifier
            | Javascript::String
            | Javascript::String2
            | Javascript::Number
            | Javascript::True
            | Javascript::False
            | Javascript::Null
            | Javascript::Void
            | Javascript::This
            | Javascript::Super
            | Javascript::Undefined
            | Javascript::Set
            | Javascript::Get
            | Javascript::Typeof
            | Javascript::Instanceof => HalsteadType::Operand,
            _ => HalsteadType::Unknown,
        }
    }

    get_operator!(Javascript);
}

impl Getter for TypescriptCode {
    fn get_space_kind(node: &Node) -> SpaceKind {
        match Typescript::from(node.kind_id()) {
            Typescript::FunctionExpression
            | Typescript::MethodDefinition
            | Typescript::GeneratorFunction
            | Typescript::FunctionDeclaration
            | Typescript::GeneratorFunctionDeclaration
            | Typescript::ArrowFunction => SpaceKind::Function,
            Typescript::Class | Typescript::ClassDeclaration => SpaceKind::Class,
            Typescript::InterfaceDeclaration => SpaceKind::Interface,
            Typescript::Program => SpaceKind::Unit,
            _ => SpaceKind::Unknown,
        }
    }

    fn get_func_space_name<'a>(node: &Node, code: &'a [u8]) -> Option<&'a str> {
        if let Some(name) = node.child_by_field_name("name") {
            let code = &code[name.start_byte()..name.end_byte()];
            std::str::from_utf8(code).ok()
        } else {
            // We can be in a pair: foo: function() {}
            // Or in a variable declaration: var aFun = function() {}
            if let Some(parent) = node.parent() {
                match Typescript::from(parent.kind_id()) {
                    Typescript::Pair => {
                        if let Some(name) = parent.child_by_field_name("key") {
                            let code = &code[name.start_byte()..name.end_byte()];
                            return std::str::from_utf8(code).ok();
                        }
                    }
                    Typescript::VariableDeclarator => {
                        if let Some(name) = parent.child_by_field_name("name") {
                            let code = &code[name.start_byte()..name.end_byte()];
                            return std::str::from_utf8(code).ok();
                        }
                    }
                    _ => {}
                }
            }
            Some("<anonymous>")
        }
    }

    fn get_op_type(node: &Node) -> HalsteadType {
        match Typescript::from(node.kind_id()) {
            Typescript::Export
            | Typescript::Import
            | Typescript::Import2
            | Typescript::Extends
            | Typescript::DOT
            | Typescript::From
            | Typescript::LPAREN
            | Typescript::COMMA
            | Typescript::As
            | Typescript::STAR
            | Typescript::GTGT
            | Typescript::GTGTGT
            | Typescript::COLON
            | Typescript::Return
            | Typescript::Delete
            | Typescript::Throw
            | Typescript::Break
            | Typescript::Continue
            | Typescript::If
            | Typescript::Else
            | Typescript::Switch
            | Typescript::Case
            | Typescript::Default
            | Typescript::Async
            | Typescript::For
            | Typescript::In
            | Typescript::Of
            | Typescript::While
            | Typescript::Try
            | Typescript::Catch
            | Typescript::Finally
            | Typescript::With
            | Typescript::EQ
            | Typescript::AT
            | Typescript::AMPAMP
            | Typescript::PIPEPIPE
            | Typescript::PLUS
            | Typescript::DASH
            | Typescript::DASHDASH
            | Typescript::PLUSPLUS
            | Typescript::SLASH
            | Typescript::PERCENT
            | Typescript::STARSTAR
            | Typescript::PIPE
            | Typescript::AMP
            | Typescript::LTLT
            | Typescript::TILDE
            | Typescript::LT
            | Typescript::LTEQ
            | Typescript::EQEQ
            | Typescript::BANGEQ
            | Typescript::GTEQ
            | Typescript::GT
            | Typescript::PLUSEQ
            | Typescript::BANG
            | Typescript::BANGEQEQ
            | Typescript::EQEQEQ
            | Typescript::DASHEQ
            | Typescript::STAREQ
            | Typescript::SLASHEQ
            | Typescript::PERCENTEQ
            | Typescript::STARSTAREQ
            | Typescript::GTGTEQ
            | Typescript::GTGTGTEQ
            | Typescript::LTLTEQ
            | Typescript::AMPEQ
            | Typescript::CARET
            | Typescript::CARETEQ
            | Typescript::PIPEEQ
            | Typescript::Yield
            | Typescript::LBRACK
            | Typescript::LBRACE
            | Typescript::Await
            | Typescript::QMARK
            | Typescript::QMARKQMARK
            | Typescript::New
            | Typescript::Let
            | Typescript::Var
            | Typescript::Const
            | Typescript::Function
            | Typescript::FunctionExpression
            | Typescript::SEMI => HalsteadType::Operator,
            Typescript::Identifier
            | Typescript::NestedIdentifier
            | Typescript::MemberExpression
            | Typescript::PropertyIdentifier
            | Typescript::String
            | Typescript::Number
            | Typescript::True
            | Typescript::False
            | Typescript::Null
            | Typescript::Void
            | Typescript::This
            | Typescript::Super
            | Typescript::Undefined
            | Typescript::Set
            | Typescript::Get
            | Typescript::Typeof
            | Typescript::Instanceof => HalsteadType::Operand,
            _ => HalsteadType::Unknown,
        }
    }

    get_operator!(Typescript);
}

impl Getter for TsxCode {
    fn get_space_kind(node: &Node) -> SpaceKind {
        match Tsx::from(node.kind_id()) {
            Tsx::FunctionExpression
            | Tsx::MethodDefinition
            | Tsx::GeneratorFunction
            | Tsx::FunctionDeclaration
            | Tsx::GeneratorFunctionDeclaration
            | Tsx::ArrowFunction => SpaceKind::Function,
            Tsx::Class | Tsx::ClassDeclaration => SpaceKind::Class,
            Tsx::InterfaceDeclaration => SpaceKind::Interface,
            Tsx::Program => SpaceKind::Unit,
            _ => SpaceKind::Unknown,
        }
    }

    fn get_func_space_name<'a>(node: &Node, code: &'a [u8]) -> Option<&'a str> {
        if let Some(name) = node.child_by_field_name("name") {
            let code = &code[name.start_byte()..name.end_byte()];
            std::str::from_utf8(code).ok()
        } else {
            // We can be in a pair: foo: function() {}
            // Or in a variable declaration: var aFun = function() {}
            if let Some(parent) = node.parent() {
                match Tsx::from(parent.kind_id()) {
                    Tsx::Pair => {
                        if let Some(name) = parent.child_by_field_name("key") {
                            let code = &code[name.start_byte()..name.end_byte()];
                            return std::str::from_utf8(code).ok();
                        }
                    }
                    Tsx::VariableDeclarator => {
                        if let Some(name) = parent.child_by_field_name("name") {
                            let code = &code[name.start_byte()..name.end_byte()];
                            return std::str::from_utf8(code).ok();
                        }
                    }
                    _ => {}
                }
            }
            Some("<anonymous>")
        }
    }

    fn get_op_type(node: &Node) -> HalsteadType {
        match Tsx::from(node.kind_id()) {
            Tsx::Export
            | Tsx::Import
            | Tsx::Import2
            | Tsx::Extends
            | Tsx::DOT
            | Tsx::From
            | Tsx::LPAREN
            | Tsx::COMMA
            | Tsx::As
            | Tsx::STAR
            | Tsx::GTGT
            | Tsx::GTGTGT
            | Tsx::COLON
            | Tsx::Return
            | Tsx::Delete
            | Tsx::Throw
            | Tsx::Break
            | Tsx::Continue
            | Tsx::If
            | Tsx::Else
            | Tsx::Switch
            | Tsx::Case
            | Tsx::Default
            | Tsx::Async
            | Tsx::For
            | Tsx::In
            | Tsx::Of
            | Tsx::While
            | Tsx::Try
            | Tsx::Catch
            | Tsx::Finally
            | Tsx::With
            | Tsx::EQ
            | Tsx::AT
            | Tsx::AMPAMP
            | Tsx::PIPEPIPE
            | Tsx::PLUS
            | Tsx::DASH
            | Tsx::DASHDASH
            | Tsx::PLUSPLUS
            | Tsx::SLASH
            | Tsx::PERCENT
            | Tsx::STARSTAR
            | Tsx::PIPE
            | Tsx::AMP
            | Tsx::LTLT
            | Tsx::TILDE
            | Tsx::LT
            | Tsx::LTEQ
            | Tsx::EQEQ
            | Tsx::BANGEQ
            | Tsx::GTEQ
            | Tsx::GT
            | Tsx::PLUSEQ
            | Tsx::BANG
            | Tsx::BANGEQEQ
            | Tsx::EQEQEQ
            | Tsx::DASHEQ
            | Tsx::STAREQ
            | Tsx::SLASHEQ
            | Tsx::PERCENTEQ
            | Tsx::STARSTAREQ
            | Tsx::GTGTEQ
            | Tsx::GTGTGTEQ
            | Tsx::LTLTEQ
            | Tsx::AMPEQ
            | Tsx::CARET
            | Tsx::CARETEQ
            | Tsx::PIPEEQ
            | Tsx::Yield
            | Tsx::LBRACK
            | Tsx::LBRACE
            | Tsx::Await
            | Tsx::QMARK
            | Tsx::QMARKQMARK
            | Tsx::New
            | Tsx::Let
            | Tsx::Var
            | Tsx::Const
            | Tsx::Function
            | Tsx::FunctionExpression
            | Tsx::SEMI => HalsteadType::Operator,
            Tsx::Identifier
            | Tsx::NestedIdentifier
            | Tsx::MemberExpression
            | Tsx::PropertyIdentifier
            | Tsx::String
            | Tsx::String2
            | Tsx::Number
            | Tsx::True
            | Tsx::False
            | Tsx::Null
            | Tsx::Void
            | Tsx::This
            | Tsx::Super
            | Tsx::Undefined
            | Tsx::Set
            | Tsx::Get
            | Tsx::Typeof
            | Tsx::Instanceof => HalsteadType::Operand,
            _ => HalsteadType::Unknown,
        }
    }

    get_operator!(Tsx);
}

impl Getter for JavaCode {
    fn get_space_kind(node: &Node) -> SpaceKind {
        match Java::from(node.kind_id()) {
            Java::ClassDeclaration => SpaceKind::Class,
            Java::MethodDeclaration | Java::ConstructorDeclaration | Java::LambdaExpression => {
                SpaceKind::Function
            }
            Java::InterfaceDeclaration => SpaceKind::Interface,
            Java::Program => SpaceKind::Unit,
            _ => SpaceKind::Unknown,
        }
    }

    fn get_op_type(node: &Node) -> HalsteadType {
        // Some guides that informed grammar choice for Halstead
        // keywords, operators, literals: https://docs.oracle.com/javase/specs/jls/se18/html/jls-3.html#jls-3.12
        // https://www.geeksforgeeks.org/software-engineering-halsteads-software-metrics/
        match Java::from(node.kind_id()) {
            // Operator: control flow
            Java::If | Java::Else | Java::Switch | Java::Case | Java::Try | Java::Catch | Java::Throw | Java::Throws | Java::Throws2 | Java::For | Java::While
            | Java::Continue | Java::Break | Java::Do | Java::Finally
            // Operator: keywords
            | Java::New | Java::Return | Java::Default | Java::Abstract | Java::Assert | Java::Instanceof | Java::Extends | Java::Final
            | Java::Implements | Java::Transient | Java::Synchronized | Java::Super | Java::This | Java::VoidType
            // Operator: brackets, comma and terminators (separators)
            | Java::SEMI | Java::COMMA | Java::COLONCOLON | Java::LBRACE | Java::LBRACK | Java::LPAREN
            // Operator: operators
            | Java::EQ | Java::LT | Java::GT | Java::BANG | Java::TILDE | Java::QMARK | Java::COLON // no grammar for the lambda operator ->
            | Java::EQEQ | Java::LTEQ | Java::GTEQ | Java::BANGEQ | Java::AMPAMP | Java::PIPEPIPE | Java::PLUSPLUS | Java::DASHDASH
            | Java::PLUS | Java::DASH | Java::STAR | Java::SLASH | Java::AMP | Java::PIPE | Java::CARET | Java::PERCENT | Java::LTLT | Java::GTGT | Java::GTGTGT
            | Java::PLUSEQ | Java::DASHEQ | Java::STAREQ | Java::SLASHEQ | Java::AMPEQ | Java::PIPEEQ | Java::CARETEQ | Java::PERCENTEQ | Java::LTLTEQ
            | Java::GTGTEQ | Java::GTGTGTEQ
            // Primitive types
            | Java::Int | Java::Float => HalsteadType::Operator,
            // Operands: variables, constants, literals
            Java::Identifier | Java::NullLiteral | Java::ClassLiteral | Java::StringLiteral | Java::CharacterLiteral
            | Java::HexIntegerLiteral | Java::OctalIntegerLiteral | Java::BinaryIntegerLiteral
            | Java::DecimalIntegerLiteral | Java::HexFloatingPointLiteral | Java::DecimalFloatingPointLiteral => {
                HalsteadType::Operand
            }
            _ => HalsteadType::Unknown,
        }
    }

    // Not `get_operator!`: Java needs `void` spelled out on top of what the
    // macro writes.
    #[inline(always)]
    fn get_operator_id_as_str(id: u16) -> &'static str {
        let typ = id.into();
        match typ {
            Java::LPAREN => "()",
            Java::LBRACK => "[]",
            Java::LBRACE => "{}",
            Java::VoidType => "void",
            _ => typ.into(),
        }
    }
}

impl Getter for RustCode {
    fn get_func_space_name<'a>(node: &Node, code: &'a [u8]) -> Option<&'a str> {
        // we're in a function or in a class or an impl
        // for an impl: we've  'impl ... type {...'
        if let Some(name) = node
            .child_by_field_name("name")
            .or_else(|| node.child_by_field_name("type"))
        {
            let code = &code[name.start_byte()..name.end_byte()];
            std::str::from_utf8(code).ok()
        } else {
            Some("<anonymous>")
        }
    }

    fn get_space_kind(node: &Node) -> SpaceKind {
        match Rust::from(node.kind_id()) {
            Rust::FunctionItem | Rust::ClosureExpression => SpaceKind::Function,
            Rust::TraitItem => SpaceKind::Trait,
            Rust::ImplItem => SpaceKind::Impl,
            Rust::SourceFile => SpaceKind::Unit,
            _ => SpaceKind::Unknown,
        }
    }

    fn get_op_type(node: &Node) -> HalsteadType {
        match Rust::from(node.kind_id()) {
            // `||` is treated as an operator only if it's part of a binary expression.
            // This prevents misclassification inside macros where closures without arguments (e.g., `let closure = || { /* ... */ };`)
            // are not recognized as `ClosureExpression` and their `||` node is identified as `PIPEPIPE` instead of `ClosureParameters`.
            //
            // Similarly, exclude `/` when it corresponds to the third slash in `///` (`OuterDocCommentMarker`)
            Rust::PIPEPIPE | Rust::SLASH => match node.parent() {
                Some(parent) if parent.kind_id() == Rust::BinaryExpression as u16 => {
                    HalsteadType::Operator
                }
                _ => HalsteadType::Unknown,
            },
            // Ensure `!` is counted as an operator unless it belongs to an `InnerDocCommentMarker` `//!`
            Rust::BANG => match node.parent() {
                Some(parent) if parent.kind_id() != Rust::InnerDocCommentMarker as u16 => {
                    HalsteadType::Operator
                }
                _ => HalsteadType::Unknown,
            },
            Rust::LPAREN
            | Rust::LBRACE
            | Rust::LBRACK
            | Rust::EQGT
            | Rust::PLUS
            | Rust::STAR
            | Rust::Async
            | Rust::Await
            | Rust::Continue
            | Rust::For
            | Rust::If
            | Rust::Let
            | Rust::Loop
            | Rust::Match
            | Rust::Return
            | Rust::Unsafe
            | Rust::While
            | Rust::EQ
            | Rust::COMMA
            | Rust::DASHGT
            | Rust::QMARK
            | Rust::LT
            | Rust::GT
            | Rust::AMP
            | Rust::MutableSpecifier
            | Rust::DOTDOT
            | Rust::DOTDOTEQ
            | Rust::DASH
            | Rust::AMPAMP
            | Rust::PIPE
            | Rust::CARET
            | Rust::EQEQ
            | Rust::BANGEQ
            | Rust::LTEQ
            | Rust::GTEQ
            | Rust::LTLT
            | Rust::GTGT
            | Rust::PERCENT
            | Rust::PLUSEQ
            | Rust::DASHEQ
            | Rust::STAREQ
            | Rust::SLASHEQ
            | Rust::PERCENTEQ
            | Rust::AMPEQ
            | Rust::PIPEEQ
            | Rust::CARETEQ
            | Rust::LTLTEQ
            | Rust::GTGTEQ
            | Rust::Move
            | Rust::DOT
            | Rust::PrimitiveType
            | Rust::Fn
            | Rust::SEMI => HalsteadType::Operator,
            Rust::Identifier
            | Rust::StringLiteral
            | Rust::RawStringLiteral
            | Rust::IntegerLiteral
            | Rust::FloatLiteral
            | Rust::BooleanLiteral
            | Rust::Zelf
            | Rust::CharLiteral
            | Rust::UNDERSCORE => HalsteadType::Operand,
            _ => HalsteadType::Unknown,
        }
    }

    get_operator!(Rust);
}

impl Getter for CppCode {
    fn get_func_space_name<'a>(node: &Node, code: &'a [u8]) -> Option<&'a str> {
        let name = if matches!(
            Cpp::from(node.kind_id()),
            Cpp::FunctionDefinition
                | Cpp::FunctionDefinition2
                | Cpp::FunctionDefinition3
                | Cpp::FunctionDefinition4
        ) {
            crate::cpp::function_name(*node)
        } else {
            node.child_by_field_name("name")
        };
        name.map_or(Some("<anonymous>"), |name| {
            std::str::from_utf8(&code[name.start_byte()..name.end_byte()]).ok()
        })
    }

    fn get_space_kind(node: &Node) -> SpaceKind {
        match Cpp::from(node.kind_id()) {
            Cpp::FunctionDefinition
            | Cpp::FunctionDefinition2
            | Cpp::FunctionDefinition3
            | Cpp::FunctionDefinition4
            | Cpp::LambdaExpression => SpaceKind::Function,
            Cpp::StructSpecifier | Cpp::UnionSpecifier => SpaceKind::Struct,
            Cpp::ClassSpecifier => SpaceKind::Class,
            Cpp::NamespaceDefinition => SpaceKind::Namespace,
            Cpp::TranslationUnit => SpaceKind::Unit,
            _ => SpaceKind::Unknown,
        }
    }

    fn get_op_type(node: &Node) -> HalsteadType {
        match Cpp::from(node.kind_id()) {
            Cpp::DOT
            | Cpp::LPAREN
            | Cpp::LPAREN2
            | Cpp::COMMA
            | Cpp::STAR
            | Cpp::GTGT
            | Cpp::COLON
            | Cpp::SEMI
            | Cpp::Return
            | Cpp::Break
            | Cpp::Continue
            | Cpp::If
            | Cpp::Else
            | Cpp::Switch
            | Cpp::Case
            | Cpp::Default
            | Cpp::For
            | Cpp::While
            | Cpp::Goto
            | Cpp::Do
            | Cpp::Delete
            | Cpp::New
            | Cpp::Try2
            | Cpp::Try
            | Cpp::Catch
            | Cpp::Throw
            | Cpp::EQ
            | Cpp::AMPAMP
            | Cpp::PIPEPIPE
            | Cpp::DASH
            | Cpp::DASHDASH
            | Cpp::DASHGT
            | Cpp::PLUS
            | Cpp::PLUSPLUS
            | Cpp::SLASH
            | Cpp::PERCENT
            | Cpp::PIPE
            | Cpp::AMP
            | Cpp::LTLT
            | Cpp::TILDE
            | Cpp::LT
            | Cpp::LTEQ
            | Cpp::EQEQ
            | Cpp::BANGEQ
            | Cpp::GTEQ
            | Cpp::GT
            | Cpp::GT2
            | Cpp::PLUSEQ
            | Cpp::DASHEQ
            | Cpp::BANG
            | Cpp::STAREQ
            | Cpp::SLASHEQ
            | Cpp::PERCENTEQ
            | Cpp::GTGTEQ
            | Cpp::LTLTEQ
            | Cpp::AMPEQ
            | Cpp::CARET
            | Cpp::CARETEQ
            | Cpp::PIPEEQ
            | Cpp::LBRACK
            | Cpp::LBRACE
            | Cpp::QMARK
            | Cpp::COLONCOLON
            | Cpp::PrimitiveType
            | Cpp::SizedTypeSpecifier
            | Cpp::Sizeof
            | Cpp::And
            | Cpp::Or
            | Cpp::Not
            | Cpp::Bitand
            | Cpp::Bitor
            | Cpp::Xor
            | Cpp::Compl
            | Cpp::AndEq
            | Cpp::OrEq
            | Cpp::XorEq
            | Cpp::NotEq => HalsteadType::Operator,
            Cpp::Identifier
            | Cpp::TypeIdentifier
            | Cpp::FieldIdentifier
            | Cpp::RawStringLiteral
            | Cpp::StringLiteral
            | Cpp::CharLiteral
            | Cpp::NumberLiteral
            | Cpp::True
            | Cpp::False
            | Cpp::Null
            | Cpp::DOTDOTDOT => HalsteadType::Operand,
            Cpp::NamespaceIdentifier => match node.parent() {
                Some(parent) if parent.kind_id() == Cpp::NamespaceDefinition as u16 => {
                    HalsteadType::Operand
                }
                _ => HalsteadType::Unknown,
            },
            _ => HalsteadType::Unknown,
        }
    }

    get_operator!(Cpp);
}
