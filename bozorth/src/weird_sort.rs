struct Cell {
    index: usize,
    value: i32,
}

fn select_pivot(v: &[Cell], left: usize, right: usize) -> usize {
    let midpoint = (left + right) / 2;
    let ileft = v[left].value;
    let imidpoint = v[midpoint].value;
    let iright = v[right].value;

    if ileft <= imidpoint {
        if imidpoint <= iright {
            midpoint
        } else if iright > ileft {
            right
        } else {
            left
        }
    } else if ileft < iright {
        left
    } else if iright < imidpoint {
        midpoint
    } else {
        right
    }
}

fn partition_dec(
    cells: &mut [Cell],
    mut pivot: usize,
    mut left: usize,
    mut right: usize,
) -> (usize, usize, usize, usize, usize, usize) {
    let left_begin = left;
    let right_end = right;

    loop {
        if left < pivot {
            if cells[left].value < cells[pivot].value {
                cells.swap(left, pivot);
                pivot = left;
            } else {
                left += 1
            }
        } else if right > pivot {
            if cells[right].value > cells[pivot].value {
                cells.swap(right, pivot);
                pivot = right;
                left += 1;
            } else {
                right -= 1;
            }
        } else {
            let left_end = pivot.checked_sub(1).unwrap_or(0);
            let right_begin = pivot + 1;
            let left_len = left_end + 1 - left_begin;
            let right_len = right_end + 1 - right_begin;
            return (
                left_begin,
                left_end,
                left_len,
                right_begin,
                right_end,
                right_len,
            );
        }
    }
}

fn qsort_decreasing(cells: &mut [Cell], left: usize, right: usize) {
    let mut stack = vec![];
    stack.push(left);
    stack.push(right);
    while !stack.is_empty() {
        let right = stack.pop().unwrap();
        let left = stack.pop().unwrap();
        if left < right {
            let pivot = select_pivot(cells, left, right);
            let (left_begin, left_end, left_len, right_begin, right_end, right_len) =
                partition_dec(cells, pivot, left, right);
            if left_len > right_len {
                stack.push(left_begin);
                stack.push(left_end);
                stack.push(right_begin);
                stack.push(right_end);
            } else {
                stack.push(right_begin);
                stack.push(right_end);
                stack.push(left_begin);
                stack.push(left_end);
            }
        }
    }
}

pub(crate) fn sort_order_decreasing(values: &[i32], order: &mut [usize]) {
    assert_eq!(values.len(), order.len());

    let mut cells: Vec<Cell> = values
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, value)| Cell { index, value })
        .collect();

    qsort_decreasing(&mut cells, 0, values.len() - 1);

    for i in 0..order.len() {
        order[i] = cells[i].index;
    }
}
