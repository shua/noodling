#![feature(slice_patterns)]
/// an attempt to implement the Ghosts of Departed Proofs idea in rust
/// https://kataskeue.com/gdp.pdf

#[macro_use]
pub mod named {
    use core::marker::PhantomData;
    #[derive(Clone)]
	// can't use newtype with phantom type from haskell, because rust will complain about an unused type paramater
	// pub type Named<Name, A> = A;
    pub struct Named<Name, A>(pub A, PhantomData<Name>);

    // not quite the same type as name in the paper
    // name :: a -> (forall name. (a ~~ name) -> t) -> t
    // name x k :: k (coerce x)
    //
    // rank-N types don't exist in rust yet https://github.com/rust-lang/rfcs/issues/1481 but might look like
    // fn name<A, K, T>(x: A, k: K) -> T
    //    where for<Name> K: Fn(Named<Name, A>) -> T
    //
    // The combination of haskell's phantom types and coercion is being mimiced by using rust's PhantomData, and closure types.
    // I'm assuming that (coerce x) above is creating a new, distinct type for name in (a ~~ name).
    // I need to get a new, distinct type with every invocation of name, and the only way I know to get rust to do that is closures.
    // So using the dummy || () closure in the name! macro means that the type of every named var passed to the k function is Named<closure@some_place_in_code, A>, which should be distinct.
    //
    // the error messages aren't great, but you end up with the desired result of compiler errors for mismatched Named vars
    pub fn name<Name, T, A, K: Fn(Named<Name, A>) -> T>(_: Name, x: A, k: K) -> T {
        k(Named(x, PhantomData))
    }

    #[macro_export]
    macro_rules! name {
        ( $x:expr, $k:expr ) => {
            named::name(|| (), $x, $k)
        };
    }
}

pub mod list_util {
    use core::fmt::Debug;
    use std::cmp::Ordering;
    pub fn merge_by<A, Comp>(mut comp: Comp, xs: Vec<A>, ys: Vec<A>) -> Vec<A>
    where
        Comp: FnMut(&A, &A) -> Ordering,
        A: Clone + Debug,
    {
        if xs.is_empty() {
            return ys;
        }
        if ys.is_empty() {
            return xs;
        }

        let x = xs[0].clone();
        let y = ys[0].clone();
        match comp(&x, &y) {
            Ordering::Greater => {
                let mut zs = vec![y];
                let zs1 = &mut merge_by(comp, xs, ys[1..].to_vec());
                zs.append(zs1);
                zs
            }
            _ => {
                let mut zs = vec![x];
                let zs1 = &mut merge_by(comp, xs[1..].to_vec(), ys);
                zs.append(zs1);
                zs
            }
        }
    }
}

mod sorted {
    use super::list_util as U;
    use super::named::{Named, self};
    use core::fmt::Debug;
    use core::marker::PhantomData;
    use std::cmp::Ordering;

    #[derive(Debug)]
    pub struct SortedBy<Comp, A>(pub A, PhantomData<Comp>);

    pub fn sort_by<Comp, A, CompFn>(comp: Named<Comp, CompFn>, mut xs: Vec<A>) -> SortedBy<Comp, Vec<A>>
    where
        CompFn: FnMut(&A, &A) -> Ordering,
    {
        xs.sort_unstable_by(comp.0);
        SortedBy(xs, PhantomData)
    }

    pub fn merge_by<Comp, A, CompFn>(
        comp: Named<Comp, CompFn>,
        xs: SortedBy<Comp, Vec<A>>,
        ys: SortedBy<Comp, Vec<A>>,
    ) -> SortedBy<Comp, Vec<A>>
    where
        CompFn: FnMut(&A, &A) -> Ordering,
        A: Clone + Debug,
    {
        SortedBy(U::merge_by(comp.0, xs.0, ys.0), PhantomData)
    }

    fn greater_than<A: Ord>(a: &A, b: &A) -> Ordering {
        if a > b {
            Ordering::Greater
        } else if a < b {
            Ordering::Less
        } else {
            Ordering::Equal
        }
    }

    pub fn sorted_main() {
        let xs: Vec<i64> = vec![3, 1, 2];
        let ys: Vec<i64> = vec![2, 1, 3];
        name!(greater_than, |gt| {
            let xs = sort_by(gt.clone(), xs.clone());
            let ys = sort_by(gt.clone(), ys.clone());
            println!("{:?}", merge_by(gt, xs, ys).0);
        })
    }

    #[test]
    /// if we let the users define their own Name type, then they can make two things equal
    fn user_defd_names() {
        type Simon = ();
        fn less_than<A: Ord>(a: &A, b: &A) -> Ordering {
            greater_than(a, b).reverse()
        }
        let up: Named<Simon, fn(&i64, &i64) -> Ordering> = Named(greater_than, PhantomData);
        let down: Named<Simon, fn(&i64, &i64) -> Ordering> = Named(less_than, PhantomData);

        let list1 = sort_by(up.clone(), vec![1, 2, 3]);
        let list2 = sort_by(down, vec![1, 2, 3]);

        let merged = merge_by(up, list1, list2);
        assert_eq!(merged.0, vec![1, 2, 3, 3, 2, 1]);
    }

    #[test]
    fn defd_names() {
        fn less_than<A: Ord>(a: &A, b: &A) -> Ordering {
            greater_than(a, b).reverse()
        }
        name!(greater_than, |up| {
            name!(less_than, |down| {
                #[allow(unused)]
                let list1 = sort_by(up.clone(), vec![1, 2, 3]);
                #[allow(unused)]
                let list2 = sort_by(down, vec![1, 2, 3]);
                // merge_by(up, list1, list2)
                // uncommenting the above line will resurt in a compiler error as desired
            })
        })
    }
}

mod st {
	#[macro_use]
	use super::named::Named;

	type Region = ();
	const Region: Region = ();
	struct St<S, A>(Named<S, Region>, A);

	fn run_st<S, A>(action: St<S, A> -> A
}

fn main() {
	sorted::sorted_main()
}
