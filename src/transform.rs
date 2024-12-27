const CONST1: i32 = 20091;
const CONST2: i32 = 35468;

pub(crate) fn idct4x4(block: &mut [[i32; 4]; 4]) {
    let mut new_block = [[0i32; 4]; 4];
    for x in 0usize..4 {
        let a1 = block[0][x] + block[2][x];
        let b1 = block[0][x] - block[2][x];

        let t1 = (block[1][x] * CONST2) >> 16;
        let t2 = block[3][x] + ((block[3][x] * CONST1) >> 16);
        let c1 = t1 - t2;

        let t1 = block[1][x] + ((block[1][x] * CONST1) >> 16);
        let t2 = (block[3][x] * CONST2) >> 16;
        let d1 = t1 + t2;

        new_block[0][x] = a1;
        new_block[1][x] = b1;
        new_block[2][x] = c1;
        new_block[3][x] = d1;
    }
    std::mem::swap(block, &mut new_block);

    for x in 0usize..4 {
        let a1 = block[0][x];
        let b1 = block[1][x];
        let c1 = block[2][x];
        let d1 = block[3][x];

        new_block[0][x] = a1 + d1;
        new_block[1][x] = b1 + c1;
        new_block[3][x] = a1 - d1;
        new_block[2][x] = b1 - c1;
    }
    std::mem::swap(block, &mut new_block);

    for y in 0usize..4 {
        let a1 = block[y][0] + block[y][2];
        let b1 = block[y][0] - block[y][2];

        let t1 = (block[y][1] * CONST2) >> 16;
        let t2 = block[y][3] + ((block[y][3] * CONST1) >> 16);
        let c1 = t1 - t2;

        let t1 = block[y][1] + ((block[y][1] * CONST1) >> 16);
        let t2 = (block[y][3] * CONST2) >> 16;
        let d1 = t1 + t2;

        new_block[y][0] = (a1 + d1 + 4) >> 3;
        new_block[y][3] = (a1 - d1 + 4) >> 3;
        new_block[y][1] = (b1 + c1 + 4) >> 3;
        new_block[y][2] = (b1 - c1 + 4) >> 3;
    }
    std::mem::swap(block, &mut new_block);
}

// 14.3
pub(crate) fn iwht4x4(block: &mut [[i32; 4]; 4]) {
    let mut new_block = [[0i32; 4]; 4];
    for x in 0usize..4 {
        let a1 = block[0][x] + block[3][x];
        let b1 = block[1][x] + block[2][x];
        let c1 = block[1][x] - block[2][x];
        let d1 = block[0][x] - block[3][x];

        new_block[0][x] = a1 + b1;
        new_block[1][x] = c1 + d1;
        new_block[2][x] = a1 - b1;
        new_block[3][x] = d1 - c1;
    }
    std::mem::swap(block, &mut new_block);

    for y in 0usize..4 {
        let a1 = block[y][0] + block[y][3];
        let b1 = block[y][1] + block[y][2];
        let c1 = block[y][1] - block[y][2];
        let d1 = block[y][0] - block[y][3];

        let a2 = a1 + b1;
        let b2 = c1 + d1;
        let c2 = a1 - b1;
        let d2 = d1 - c1;

        new_block[y][0] = (a2 + 3) >> 3;
        new_block[y][1] = (b2 + 3) >> 3;
        new_block[y][2] = (c2 + 3) >> 3;
        new_block[y][3] = (d2 + 3) >> 3;
    }
    std::mem::swap(block, &mut new_block);
}
