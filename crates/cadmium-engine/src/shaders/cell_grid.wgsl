const CELL_GRID_BINS: u32 = CELL_GRID_SIDE * CELL_GRID_SIDE;

fn cell_slot(contact: Contact) -> u32 {
    let bin = contact.along * CELL_GRID_SIDE + contact.across;
    return cell_grids[contact.bristle * CELL_GRID_BINS + bin];
}
