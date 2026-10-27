pub type Vec3 = [f64; 3];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4 {
    rows: [[f64; 4]; 4],
}

impl Mat4 {
    pub const IDENTITY: Mat4 = Mat4 {
        rows: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };

    pub fn from_rows(rows: [[f64; 4]; 4]) -> Self {
        Mat4 { rows }
    }

    pub fn rows(&self) -> [[f64; 4]; 4] {
        self.rows
    }

    pub fn mul(&self, other: &Mat4) -> Mat4 {
        let mut rows = [[0.0; 4]; 4];
        for (row, out) in self.rows.iter().zip(rows.iter_mut()) {
            for (column, cell) in out.iter_mut().enumerate() {
                *cell = (0..4).map(|k| row[k] * other.rows[k][column]).sum();
            }
        }
        Mat4 { rows }
    }

    pub fn apply(&self, vector: [f64; 4]) -> [f64; 4] {
        self.rows
            .map(|row| row.iter().zip(vector).map(|(a, b)| a * b).sum())
    }

    pub fn column_major_f32(&self) -> [f32; 16] {
        let mut out = [0.0; 16];
        for (column, chunk) in out.chunks_mut(4).enumerate() {
            for (row, cell) in chunk.iter_mut().enumerate() {
                *cell = self.rows[row][column] as f32;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_neutral() {
        let m = Mat4::from_rows([
            [1.0, 2.0, 3.0, 4.0],
            [5.0, 6.0, 7.0, 8.0],
            [9.0, 10.0, 11.0, 12.0],
            [13.0, 14.0, 15.0, 16.0],
        ]);
        assert_eq!(m.mul(&Mat4::IDENTITY), m);
        assert_eq!(Mat4::IDENTITY.mul(&m), m);
        assert_eq!(m.apply([1.0, 0.0, 0.0, 0.0]), [1.0, 5.0, 9.0, 13.0]);
    }

    #[test]
    fn product_applies_right_then_left() {
        let scale = Mat4::from_rows([
            [2.0, 0.0, 0.0, 0.0],
            [0.0, 2.0, 0.0, 0.0],
            [0.0, 0.0, 2.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        let shift = Mat4::from_rows([
            [1.0, 0.0, 0.0, 3.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        assert_eq!(
            scale.mul(&shift).apply([1.0, 1.0, 1.0, 1.0]),
            [8.0, 2.0, 2.0, 1.0]
        );
    }

    #[test]
    fn column_major_puts_translation_last() {
        let shift = Mat4::from_rows([
            [1.0, 0.0, 0.0, 3.0],
            [0.0, 1.0, 0.0, 4.0],
            [0.0, 0.0, 1.0, 5.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        assert_eq!(&shift.column_major_f32()[12..], &[3.0, 4.0, 5.0, 1.0]);
    }
}
