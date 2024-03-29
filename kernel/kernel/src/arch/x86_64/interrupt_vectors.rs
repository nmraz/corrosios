pub const TOTAL_VECTORS: usize = 256;

pub const VECTOR_DIVIDE_ERROR: u64 = 0;
pub const VECTOR_DEBUG: u64 = 1;
pub const VECTOR_NMI: u64 = 2;
pub const VECTOR_BREAKPOINT: u64 = 3;
pub const VECTOR_OVERFLOW: u64 = 4;
pub const VECTOR_BOUND: u64 = 5;
pub const VECTOR_INVALID_OPCODE: u64 = 6;
pub const VECTOR_DEVICE_NOT_AVAIL: u64 = 7;
pub const VECTOR_DOUBLE_FAULT: u64 = 8;
pub const VECTOR_INVALID_TSS: u64 = 10;
pub const VECTOR_SEGMENT_NP: u64 = 11;
pub const VECTOR_STACK_FAULT: u64 = 12;
pub const VECTOR_GP_FAULT: u64 = 13;
pub const VECTOR_PAGE_FAULT: u64 = 14;
pub const VECTOR_FPU_ERROR: u64 = 16;
pub const VECTOR_ALIGNMENT_CHECK: u64 = 17;
pub const VECTOR_MACHINE_CHECK: u64 = 18;
pub const VECTOR_SIMD_ERROR: u64 = 19;

macro_rules! for_each_interrupt {
    ($vector:ident $(, $ctx:tt)?) => {
        // Faults/exceptions (and NMI :))
        $vector!(0 $(, $ctx)?);
        $vector!(1 $(, $ctx)?);
        $vector!(2 $(, $ctx)?);
        $vector!(3 $(, $ctx)?);
        $vector!(4 $(, $ctx)?);
        $vector!(5 $(, $ctx)?);
        $vector!(6 $(, $ctx)?);
        $vector!(7 $(, $ctx)?);
        $vector!(8 $(, $ctx)?);
        $vector!(9 $(, $ctx)?);
        $vector!(10 $(, $ctx)?);
        $vector!(11 $(, $ctx)?);
        $vector!(12 $(, $ctx)?);
        $vector!(13 $(, $ctx)?);
        $vector!(14 $(, $ctx)?);
        $vector!(15 $(, $ctx)?);
        $vector!(16 $(, $ctx)?);
        $vector!(17 $(, $ctx)?);
        $vector!(18 $(, $ctx)?);
        $vector!(19 $(, $ctx)?);
        $vector!(20 $(, $ctx)?);
        $vector!(21 $(, $ctx)?);
        $vector!(22 $(, $ctx)?);
        $vector!(23 $(, $ctx)?);
        $vector!(24 $(, $ctx)?);
        $vector!(25 $(, $ctx)?);
        $vector!(26 $(, $ctx)?);
        $vector!(27 $(, $ctx)?);
        $vector!(28 $(, $ctx)?);
        $vector!(29 $(, $ctx)?);
        $vector!(30 $(, $ctx)?);
        $vector!(31 $(, $ctx)?);

        // Generic IRQs
        $vector!(32 $(, $ctx)?);
        $vector!(33 $(, $ctx)?);
        $vector!(34 $(, $ctx)?);
        $vector!(35 $(, $ctx)?);
        $vector!(36 $(, $ctx)?);
        $vector!(37 $(, $ctx)?);
        $vector!(38 $(, $ctx)?);
        $vector!(39 $(, $ctx)?);
        $vector!(40 $(, $ctx)?);
        $vector!(41 $(, $ctx)?);
        $vector!(42 $(, $ctx)?);
        $vector!(43 $(, $ctx)?);
        $vector!(44 $(, $ctx)?);
        $vector!(45 $(, $ctx)?);
        $vector!(46 $(, $ctx)?);
        $vector!(47 $(, $ctx)?);
        $vector!(48 $(, $ctx)?);
        $vector!(49 $(, $ctx)?);
        $vector!(50 $(, $ctx)?);
        $vector!(51 $(, $ctx)?);
        $vector!(52 $(, $ctx)?);
        $vector!(53 $(, $ctx)?);
        $vector!(54 $(, $ctx)?);
        $vector!(55 $(, $ctx)?);
        $vector!(56 $(, $ctx)?);
        $vector!(57 $(, $ctx)?);
        $vector!(58 $(, $ctx)?);
        $vector!(59 $(, $ctx)?);
        $vector!(60 $(, $ctx)?);
        $vector!(61 $(, $ctx)?);
        $vector!(62 $(, $ctx)?);
        $vector!(63 $(, $ctx)?);
        $vector!(64 $(, $ctx)?);
        $vector!(65 $(, $ctx)?);
        $vector!(66 $(, $ctx)?);
        $vector!(67 $(, $ctx)?);
        $vector!(68 $(, $ctx)?);
        $vector!(69 $(, $ctx)?);
        $vector!(70 $(, $ctx)?);
        $vector!(71 $(, $ctx)?);
        $vector!(72 $(, $ctx)?);
        $vector!(73 $(, $ctx)?);
        $vector!(74 $(, $ctx)?);
        $vector!(75 $(, $ctx)?);
        $vector!(76 $(, $ctx)?);
        $vector!(77 $(, $ctx)?);
        $vector!(78 $(, $ctx)?);
        $vector!(79 $(, $ctx)?);
        $vector!(80 $(, $ctx)?);
        $vector!(81 $(, $ctx)?);
        $vector!(82 $(, $ctx)?);
        $vector!(83 $(, $ctx)?);
        $vector!(84 $(, $ctx)?);
        $vector!(85 $(, $ctx)?);
        $vector!(86 $(, $ctx)?);
        $vector!(87 $(, $ctx)?);
        $vector!(88 $(, $ctx)?);
        $vector!(89 $(, $ctx)?);
        $vector!(90 $(, $ctx)?);
        $vector!(91 $(, $ctx)?);
        $vector!(92 $(, $ctx)?);
        $vector!(93 $(, $ctx)?);
        $vector!(94 $(, $ctx)?);
        $vector!(95 $(, $ctx)?);
        $vector!(96 $(, $ctx)?);
        $vector!(97 $(, $ctx)?);
        $vector!(98 $(, $ctx)?);
        $vector!(99 $(, $ctx)?);
        $vector!(100 $(, $ctx)?);
        $vector!(101 $(, $ctx)?);
        $vector!(102 $(, $ctx)?);
        $vector!(103 $(, $ctx)?);
        $vector!(104 $(, $ctx)?);
        $vector!(105 $(, $ctx)?);
        $vector!(106 $(, $ctx)?);
        $vector!(107 $(, $ctx)?);
        $vector!(108 $(, $ctx)?);
        $vector!(109 $(, $ctx)?);
        $vector!(110 $(, $ctx)?);
        $vector!(111 $(, $ctx)?);
        $vector!(112 $(, $ctx)?);
        $vector!(113 $(, $ctx)?);
        $vector!(114 $(, $ctx)?);
        $vector!(115 $(, $ctx)?);
        $vector!(116 $(, $ctx)?);
        $vector!(117 $(, $ctx)?);
        $vector!(118 $(, $ctx)?);
        $vector!(119 $(, $ctx)?);
        $vector!(120 $(, $ctx)?);
        $vector!(121 $(, $ctx)?);
        $vector!(122 $(, $ctx)?);
        $vector!(123 $(, $ctx)?);
        $vector!(124 $(, $ctx)?);
        $vector!(125 $(, $ctx)?);
        $vector!(126 $(, $ctx)?);
        $vector!(127 $(, $ctx)?);
        $vector!(128 $(, $ctx)?);
        $vector!(129 $(, $ctx)?);
        $vector!(130 $(, $ctx)?);
        $vector!(131 $(, $ctx)?);
        $vector!(132 $(, $ctx)?);
        $vector!(133 $(, $ctx)?);
        $vector!(134 $(, $ctx)?);
        $vector!(135 $(, $ctx)?);
        $vector!(136 $(, $ctx)?);
        $vector!(137 $(, $ctx)?);
        $vector!(138 $(, $ctx)?);
        $vector!(139 $(, $ctx)?);
        $vector!(140 $(, $ctx)?);
        $vector!(141 $(, $ctx)?);
        $vector!(142 $(, $ctx)?);
        $vector!(143 $(, $ctx)?);
        $vector!(144 $(, $ctx)?);
        $vector!(145 $(, $ctx)?);
        $vector!(146 $(, $ctx)?);
        $vector!(147 $(, $ctx)?);
        $vector!(148 $(, $ctx)?);
        $vector!(149 $(, $ctx)?);
        $vector!(150 $(, $ctx)?);
        $vector!(151 $(, $ctx)?);
        $vector!(152 $(, $ctx)?);
        $vector!(153 $(, $ctx)?);
        $vector!(154 $(, $ctx)?);
        $vector!(155 $(, $ctx)?);
        $vector!(156 $(, $ctx)?);
        $vector!(157 $(, $ctx)?);
        $vector!(158 $(, $ctx)?);
        $vector!(159 $(, $ctx)?);
        $vector!(160 $(, $ctx)?);
        $vector!(161 $(, $ctx)?);
        $vector!(162 $(, $ctx)?);
        $vector!(163 $(, $ctx)?);
        $vector!(164 $(, $ctx)?);
        $vector!(165 $(, $ctx)?);
        $vector!(166 $(, $ctx)?);
        $vector!(167 $(, $ctx)?);
        $vector!(168 $(, $ctx)?);
        $vector!(169 $(, $ctx)?);
        $vector!(170 $(, $ctx)?);
        $vector!(171 $(, $ctx)?);
        $vector!(172 $(, $ctx)?);
        $vector!(173 $(, $ctx)?);
        $vector!(174 $(, $ctx)?);
        $vector!(175 $(, $ctx)?);
        $vector!(176 $(, $ctx)?);
        $vector!(177 $(, $ctx)?);
        $vector!(178 $(, $ctx)?);
        $vector!(179 $(, $ctx)?);
        $vector!(180 $(, $ctx)?);
        $vector!(181 $(, $ctx)?);
        $vector!(182 $(, $ctx)?);
        $vector!(183 $(, $ctx)?);
        $vector!(184 $(, $ctx)?);
        $vector!(185 $(, $ctx)?);
        $vector!(186 $(, $ctx)?);
        $vector!(187 $(, $ctx)?);
        $vector!(188 $(, $ctx)?);
        $vector!(189 $(, $ctx)?);
        $vector!(190 $(, $ctx)?);
        $vector!(191 $(, $ctx)?);
        $vector!(192 $(, $ctx)?);
        $vector!(193 $(, $ctx)?);
        $vector!(194 $(, $ctx)?);
        $vector!(195 $(, $ctx)?);
        $vector!(196 $(, $ctx)?);
        $vector!(197 $(, $ctx)?);
        $vector!(198 $(, $ctx)?);
        $vector!(199 $(, $ctx)?);
        $vector!(200 $(, $ctx)?);
        $vector!(201 $(, $ctx)?);
        $vector!(202 $(, $ctx)?);
        $vector!(203 $(, $ctx)?);
        $vector!(204 $(, $ctx)?);
        $vector!(205 $(, $ctx)?);
        $vector!(206 $(, $ctx)?);
        $vector!(207 $(, $ctx)?);
        $vector!(208 $(, $ctx)?);
        $vector!(209 $(, $ctx)?);
        $vector!(210 $(, $ctx)?);
        $vector!(211 $(, $ctx)?);
        $vector!(212 $(, $ctx)?);
        $vector!(213 $(, $ctx)?);
        $vector!(214 $(, $ctx)?);
        $vector!(215 $(, $ctx)?);
        $vector!(216 $(, $ctx)?);
        $vector!(217 $(, $ctx)?);
        $vector!(218 $(, $ctx)?);
        $vector!(219 $(, $ctx)?);
        $vector!(220 $(, $ctx)?);
        $vector!(221 $(, $ctx)?);
        $vector!(222 $(, $ctx)?);
        $vector!(223 $(, $ctx)?);
        $vector!(224 $(, $ctx)?);
        $vector!(225 $(, $ctx)?);
        $vector!(226 $(, $ctx)?);
        $vector!(227 $(, $ctx)?);
        $vector!(228 $(, $ctx)?);
        $vector!(229 $(, $ctx)?);
        $vector!(230 $(, $ctx)?);
        $vector!(231 $(, $ctx)?);
        $vector!(232 $(, $ctx)?);
        $vector!(233 $(, $ctx)?);
        $vector!(234 $(, $ctx)?);
        $vector!(235 $(, $ctx)?);
        $vector!(236 $(, $ctx)?);
        $vector!(237 $(, $ctx)?);
        $vector!(238 $(, $ctx)?);
        $vector!(239 $(, $ctx)?);
        $vector!(240 $(, $ctx)?);
        $vector!(241 $(, $ctx)?);
        $vector!(242 $(, $ctx)?);
        $vector!(243 $(, $ctx)?);
        $vector!(244 $(, $ctx)?);
        $vector!(245 $(, $ctx)?);
        $vector!(246 $(, $ctx)?);
        $vector!(247 $(, $ctx)?);
        $vector!(248 $(, $ctx)?);
        $vector!(249 $(, $ctx)?);
        $vector!(250 $(, $ctx)?);
        $vector!(251 $(, $ctx)?);
        $vector!(252 $(, $ctx)?);
        $vector!(253 $(, $ctx)?);
        $vector!(254 $(, $ctx)?);
        $vector!(255 $(, $ctx)?);
    };
}
