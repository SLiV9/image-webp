const CONST1: i64 = 20091;
const CONST2: i64 = 35468;

pub(crate) fn idct4x4(block: &mut [[i32; 4]; 4]) {
    let mut big_block = [[0i64; 4]; 4];
    for y in 0..4 {
        for x in 0..4 {
            big_block[y][x] = block[y][x] as i64;
        }
    }
    idct_impl_part1(&mut big_block);
    idct_impl_part2(&mut big_block);
    idct_impl_part3(&mut big_block);
    for y in 0..4 {
        for x in 0..4 {
            block[y][x] = big_block[y][x] as i32;
        }
    }
}

fn idct_impl_part1(block: &mut [[i64; 4]; 4]) {
    let mut tees = [[0i64; 4]; 4];
    for x in 0usize..4 {
        tees[x][0] = (block[1][x] * CONST2) >> 16;
        tees[x][1] = block[3][x] + ((block[3][x] * CONST1) >> 16);
        tees[x][2] = block[1][x] + ((block[1][x] * CONST1) >> 16);
        tees[x][3] = (block[3][x] * CONST2) >> 16;
    }

    let mut new_block = [[0i64; 4]; 4];
    for x in 0usize..4 {
        let a1 = block[0][x] + block[2][x];
        let b1 = block[0][x] - block[2][x];
        let c1 = tees[x][0] - tees[x][1];
        let d1 = tees[x][2] + tees[x][3];

        new_block[0][x] = a1 + d1;
        new_block[1][x] = b1 + c1;
        new_block[3][x] = a1 - d1;
        new_block[2][x] = b1 - c1;
    }
    *block = new_block;
}

fn idct_impl_part2(block: &mut [[i64; 4]; 4]) {
    let mut tees = [[0i64; 4]; 4];
    idct_impl_part2_a(&*block, &mut tees);
    idct_impl_part2_b(&*block, &mut tees);
    idct_impl_part2_f(block, &tees);
}

fn idct_impl_part2_a(block: &[[i64; 4]; 4], tees: &mut [[i64; 4]; 4]) {
    let mut cees = [[0i64; 4]; 4];
    for y in 0usize..4 {
        cees[y][0] = block[y][1] * CONST2;
        cees[y][1] = block[y][3] * CONST1;
        cees[y][2] = block[y][1] * CONST1;
        cees[y][3] = block[y][3] * CONST2;
    }

    for y in 0usize..4 {
        tees[y][0] = cees[y][0] >> 16;
        tees[y][1] = cees[y][1] >> 16;
        tees[y][2] = cees[y][2] >> 16;
        tees[y][3] = cees[y][3] >> 16;
    }
}

fn idct_impl_part2_b(block: &[[i64; 4]; 4], tees: &mut [[i64; 4]; 4]) {
    for y in 0usize..4 {
        tees[y][1] += block[y][3];
        tees[y][2] += block[y][1];
    }
}

fn idct_impl_part2_f(block: &mut [[i64; 4]; 4], tees: &[[i64; 4]; 4]) {
    let mut new_block = [[0i64; 4]; 4];
    for y in 0usize..4 {
        let a1 = block[y][0] + block[y][2];
        let b1 = block[y][0] - block[y][2];
        let c1 = tees[y][0] - tees[y][1];
        let d1 = tees[y][2] + tees[y][3];

        new_block[y][0] = a1 + d1;
        new_block[y][3] = a1 - d1;
        new_block[y][1] = b1 + c1;
        new_block[y][2] = b1 - c1;
    }
    *block = new_block;
}

fn idct_impl_part3(block: &mut [[i64; 4]; 4]) {
    let mut new_block = [[0i64; 4]; 4];
    for y in 0usize..4 {
        new_block[y][0] = (block[y][0] + 4) >> 3;
        new_block[y][1] = (block[y][1] + 4) >> 3;
        new_block[y][2] = (block[y][2] + 4) >> 3;
        new_block[y][3] = (block[y][3] + 4) >> 3;
    }
    *block = new_block;
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
    *block = new_block;
}
