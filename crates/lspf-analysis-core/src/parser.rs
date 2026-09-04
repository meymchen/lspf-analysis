use std::marker::PhantomData;
use std::path::Path;

use crate::abc::Abc;
use crate::checker::Checker;
use crate::cognitive::Cognitive;
use crate::cyclomatic::Cyclomatic;
use crate::exit::Exit;
use crate::halstead::Halstead;
use crate::loc::Loc;
use crate::mi::Mi;
use crate::nargs::NArgs;
use crate::nom::Nom;
use crate::npa::Npa;
use crate::npm::Npm;
use crate::wmc::Wmc;
use crate::working_memory::WorkingMemory;

use crate::getter::Getter;

use crate::langs::*;
use crate::node::{Node, Tree};
use crate::traits::*;

#[derive(Debug)]
pub struct Parser<
    T: LanguageInfo
        + Checker
        + Getter
        + Abc
        + Cognitive
        + Cyclomatic
        + Exit
        + Halstead
        + Loc
        + Mi
        + NArgs
        + Nom
        + Npa
        + Npm
        + Wmc
        + WorkingMemory,
> {
    code: Vec<u8>,
    tree: Tree,
    phantom: PhantomData<T>,
}

impl<
    T: 'static
        + LanguageInfo
        + Checker
        + Getter
        + Abc
        + Cognitive
        + Cyclomatic
        + Exit
        + Halstead
        + Loc
        + Mi
        + NArgs
        + Nom
        + Npa
        + Npm
        + Wmc
        + WorkingMemory,
> ParserTrait for Parser<T>
{
    type Checker = T;
    type Getter = T;
    type Cognitive = T;
    type Cyclomatic = T;
    type Halstead = T;
    type Loc = T;
    type Nom = T;
    type Mi = T;
    type NArgs = T;
    type Exit = T;
    type Wmc = T;
    type Abc = T;
    type Npm = T;
    type Npa = T;
    type WorkingMemory = T;

    fn new(code: Vec<u8>, _path: &Path) -> Self {
        let tree = Tree::new::<T>(&code);

        Self {
            code,
            tree,
            phantom: PhantomData,
        }
    }

    #[inline(always)]
    fn get_language(&self) -> LANG {
        T::get_lang()
    }

    #[inline(always)]
    fn get_root(&self) -> Node<'_> {
        self.tree.get_root()
    }

    #[inline(always)]
    fn get_code(&self) -> &[u8] {
        &self.code
    }
}
