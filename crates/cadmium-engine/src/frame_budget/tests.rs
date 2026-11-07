use super::*;

fn learnt_from(dabs: usize, gpu_ms: f64) -> DabBudget {
    let mut budget = DabBudget::default();
    for _ in 0..100 {
        budget.observe(dabs, gpu_ms);
    }
    budget
}

#[test]
fn a_frame_is_allowed_as_many_dabs_as_fit_its_target_at_the_learnt_cost() {
    let budget = learnt_from(10, 5.0);
    assert_eq!(budget.allowance(), 24);
}

#[test]
fn costly_dabs_shrink_the_allowance_but_never_below_one_dab() {
    let budget = learnt_from(1, 40.0);
    assert_eq!(budget.allowance(), 1);
}

#[test]
fn one_slow_frame_moves_the_cost_only_part_of_the_way() {
    let mut budget = learnt_from(10, 5.0);
    budget.observe(10, 50.0);
    assert!(budget.cost_per_dab_ms() > 0.5 && budget.cost_per_dab_ms() < 5.0);
}

#[test]
fn frames_without_dabs_or_without_a_time_teach_nothing() {
    let mut budget = DabBudget::default();
    budget.observe(0, 12.0);
    budget.observe(8, f64::NAN);
    budget.observe(8, 0.0);
    assert_eq!(budget, DabBudget::default());
}
