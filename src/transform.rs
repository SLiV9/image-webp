const CONST1: i64 = 20091;
const CONST2: i64 = 35468;

pub(crate) fn idct4x4(block: &mut [[i32; 4]; 4]) {
    // The intermediate results may overflow the types, so we stretch the type.
    let mut big_block = [[0i64; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            big_block[r][c] = block[r][c] as i64;
        }
    }
    idct4x4_64(&mut big_block);
    for r in 0..4 {
        for c in 0..4 {
            block[r][c] = big_block[r][c] as i32;
        }
    }
}

fn idct4x4_64(block: &mut [[i64; 4]; 4]) {
    let mut new_block = [[0i64; 4]; 4];
    for c in 0usize..4 {
        let a1 = block[0][c] + block[2][c];
        let b1 = block[0][c] - block[2][c];

        let t1 = (block[1][c] * CONST2) >> 16;
        let t2 = block[3][c] + ((block[3][c] * CONST1) >> 16);
        let c1 = t1 - t2;

        let t1 = block[1][c] + ((block[1][c] * CONST1) >> 16);
        let t2 = (block[3][c] * CONST2) >> 16;
        let d1 = t1 + t2;

        new_block[0][c] = a1 + d1;
        new_block[1][c] = b1 + c1;
        new_block[3][c] = a1 - d1;
        new_block[2][c] = b1 - c1;
    }
    std::mem::swap(block, &mut new_block);

    for r in 0usize..4 {
        let a1 = block[r][0] + block[r][2];
        let b1 = block[r][0] - block[r][2];

        let t1 = (block[r][1] * CONST2) >> 16;
        let t2 = block[r][3] + ((block[r][3] * CONST1) >> 16);
        let c1 = t1 - t2;

        let t1 = block[r][1] + ((block[r][1] * CONST1) >> 16);
        let t2 = (block[r][3] * CONST2) >> 16;
        let d1 = t1 + t2;

        new_block[r][0] = (a1 + d1 + 4) >> 3;
        new_block[r][3] = (a1 - d1 + 4) >> 3;
        new_block[r][1] = (b1 + c1 + 4) >> 3;
        new_block[r][2] = (b1 - c1 + 4) >> 3;
    }
    std::mem::swap(block, &mut new_block);
}

// 14.3
pub(crate) fn iwht4x4(block: &mut [[i32; 4]; 4]) {
    let mut new_block = [[0i32; 4]; 4];
    for c in 0usize..4 {
        let a1 = block[0][c] + block[3][c];
        let b1 = block[1][c] + block[2][c];
        let c1 = block[1][c] - block[2][c];
        let d1 = block[0][c] - block[3][c];

        new_block[0][c] = a1 + b1;
        new_block[1][c] = c1 + d1;
        new_block[2][c] = a1 - b1;
        new_block[3][c] = d1 - c1;
    }
    std::mem::swap(block, &mut new_block);

    for r in 0usize..4 {
        let a1 = block[r][0] + block[r][3];
        let b1 = block[r][1] + block[r][2];
        let c1 = block[r][1] - block[r][2];
        let d1 = block[r][0] - block[r][3];

        let a2 = a1 + b1;
        let b2 = c1 + d1;
        let c2 = a1 - b1;
        let d2 = d1 - c1;

        new_block[r][0] = (a2 + 3) >> 3;
        new_block[r][1] = (b2 + 3) >> 3;
        new_block[r][2] = (c2 + 3) >> 3;
        new_block[r][3] = (d2 + 3) >> 3;
    }
    std::mem::swap(block, &mut new_block);
}
