use std::collections::VecDeque;

/// The pixels touching `index`, sides and corners, on a canvas `width` pixels wide.
fn neighbours(index: usize, width: usize, len: usize) -> impl Iterator<Item = usize> {
    let (width, height) = (width as i64, (len / width) as i64);
    let (x, y) = (index as i64 % width, index as i64 / width);
    (-1..=1)
        .flat_map(move |dy| (-1..=1).map(move |dx| (x + dx, y + dy)))
        .filter(move |&(nx, ny)| {
            (nx, ny) != (x, y) && (0..width).contains(&nx) && (0..height).contains(&ny)
        })
        .map(move |(nx, ny)| (ny * width + nx) as usize)
}

/// Marks every `open` pixel reached from `seeds` as seen and returns how many it reached.
pub fn flood(
    open: &[bool],
    seen: &mut [bool],
    width: usize,
    seeds: impl IntoIterator<Item = usize>,
) -> usize {
    let mut queue: VecDeque<usize> = seeds.into_iter().filter(|&seed| open[seed]).collect();
    for &seed in &queue {
        seen[seed] = true;
    }
    let mut reached = 0;
    while let Some(index) = queue.pop_front() {
        reached += 1;
        let fresh: Vec<usize> = neighbours(index, width, open.len())
            .filter(|&next| open[next] && !seen[next])
            .collect();
        for next in fresh {
            seen[next] = true;
            queue.push_back(next);
        }
    }
    reached
}

/// The area of every connected patch of `open` pixels not seen yet.
pub fn patch_areas_px2(open: &[bool], seen: &mut [bool], width: usize) -> Vec<usize> {
    let mut areas = Vec::new();
    for index in 0..open.len() {
        if open[index] && !seen[index] {
            areas.push(flood(open, seen, width, [index]));
        }
    }
    areas
}
