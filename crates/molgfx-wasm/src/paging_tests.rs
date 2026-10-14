use crate::paging::{Paged, PagingError};

#[test]
fn pages_assemble_one_contiguous_column_in_order() {
    let source: Vec<u32> = (0..10_000).collect();
    let mut paged = Paged::new(source.len()).map_err(|error| error.to_string());
    let Ok(paged) = paged.as_mut() else {
        panic!("reservation failed");
    };
    for page in source.chunks(777) {
        assert_eq!(paged.push(page), Ok(()));
    }
    assert!(paged.is_complete());
    assert_eq!(paged.finish().as_deref(), Ok(source.as_slice()));
}

#[test]
fn a_page_past_the_declared_total_is_refused_without_growth() {
    let Ok(mut paged) = Paged::<f32>::new(3) else {
        panic!("reservation failed");
    };
    assert_eq!(paged.push(&[1.0, 2.0]), Ok(()));
    assert_eq!(
        paged.push(&[3.0, 4.0]),
        Err(PagingError::Overflow { total: 3 })
    );
    assert_eq!(paged.as_slice(), &[1.0, 2.0]);
}

#[test]
fn finishing_early_reports_what_arrived() {
    let Ok(mut paged) = Paged::<u8>::new(4) else {
        panic!("reservation failed");
    };
    assert_eq!(paged.push(&[1]), Ok(()));
    assert_eq!(
        paged.finish(),
        Err(PagingError::Incomplete {
            received: 1,
            total: 4
        })
    );
}

#[test]
fn an_unreservable_total_is_a_value_not_an_abort() {
    assert_eq!(
        Paged::<u64>::new(usize::MAX).map(|_| ()),
        Err(PagingError::Reserve { total: usize::MAX })
    );
}
