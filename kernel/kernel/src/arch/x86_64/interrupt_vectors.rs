macro_rules! for_each_interrupt {
    ($vector:ident) => {
        $vector!(0, de);
        $vector!(1, db);
        $vector!(2, nmi);
        $vector!(3, bp);
        $vector!(4, of);
        $vector!(5, br);
        $vector!(6, ud);
        $vector!(7, nm);
        $vector!(8, df);
        $vector!(9, reserved);
        $vector!(10, ts);
        $vector!(11, np);
        $vector!(12, ss);
        $vector!(13, gp);
        $vector!(14, pf);
        $vector!(15, reserved);
        $vector!(16, mf);
        $vector!(17, ac);
        $vector!(18, mc);
        $vector!(19, xm);
        $vector!(20, ve);
        $vector!(21, cp);
        $vector!(22, reserved);
        $vector!(23, reserved);
        $vector!(24, reserved);
        $vector!(25, reserved);
        $vector!(26, reserved);
        $vector!(27, reserved);
        $vector!(28, reserved);
        $vector!(29, reserved);
        $vector!(30, reserved);
        $vector!(31, reserved);

        $vector!(32, irq);
        $vector!(33, irq);
        $vector!(34, irq);
        $vector!(35, irq);
        $vector!(36, irq);
        $vector!(37, irq);
        $vector!(38, irq);
        $vector!(39, irq);
        $vector!(40, irq);
        $vector!(41, irq);
        $vector!(42, irq);
        $vector!(43, irq);
        $vector!(44, irq);
        $vector!(45, irq);
        $vector!(46, irq);
        $vector!(47, irq);
        $vector!(48, irq);
        $vector!(49, irq);
        $vector!(50, irq);
        $vector!(51, irq);
        $vector!(52, irq);
        $vector!(53, irq);
        $vector!(54, irq);
        $vector!(55, irq);
        $vector!(56, irq);
        $vector!(57, irq);
        $vector!(58, irq);
        $vector!(59, irq);
        $vector!(60, irq);
        $vector!(61, irq);
        $vector!(62, irq);
        $vector!(63, irq);
        $vector!(64, irq);
        $vector!(65, irq);
        $vector!(66, irq);
        $vector!(67, irq);
        $vector!(68, irq);
        $vector!(69, irq);
        $vector!(70, irq);
        $vector!(71, irq);
        $vector!(72, irq);
        $vector!(73, irq);
        $vector!(74, irq);
        $vector!(75, irq);
        $vector!(76, irq);
        $vector!(77, irq);
        $vector!(78, irq);
        $vector!(79, irq);
        $vector!(80, irq);
        $vector!(81, irq);
        $vector!(82, irq);
        $vector!(83, irq);
        $vector!(84, irq);
        $vector!(85, irq);
        $vector!(86, irq);
        $vector!(87, irq);
        $vector!(88, irq);
        $vector!(89, irq);
        $vector!(90, irq);
        $vector!(91, irq);
        $vector!(92, irq);
        $vector!(93, irq);
        $vector!(94, irq);
        $vector!(95, irq);
        $vector!(96, irq);
        $vector!(97, irq);
        $vector!(98, irq);
        $vector!(99, irq);
        $vector!(100, irq);
        $vector!(101, irq);
        $vector!(102, irq);
        $vector!(103, irq);
        $vector!(104, irq);
        $vector!(105, irq);
        $vector!(106, irq);
        $vector!(107, irq);
        $vector!(108, irq);
        $vector!(109, irq);
        $vector!(110, irq);
        $vector!(111, irq);
        $vector!(112, irq);
        $vector!(113, irq);
        $vector!(114, irq);
        $vector!(115, irq);
        $vector!(116, irq);
        $vector!(117, irq);
        $vector!(118, irq);
        $vector!(119, irq);
        $vector!(120, irq);
        $vector!(121, irq);
        $vector!(122, irq);
        $vector!(123, irq);
        $vector!(124, irq);
        $vector!(125, irq);
        $vector!(126, irq);
        $vector!(127, irq);
        $vector!(128, irq);
        $vector!(129, irq);
        $vector!(130, irq);
        $vector!(131, irq);
        $vector!(132, irq);
        $vector!(133, irq);
        $vector!(134, irq);
        $vector!(135, irq);
        $vector!(136, irq);
        $vector!(137, irq);
        $vector!(138, irq);
        $vector!(139, irq);
        $vector!(140, irq);
        $vector!(141, irq);
        $vector!(142, irq);
        $vector!(143, irq);
        $vector!(144, irq);
        $vector!(145, irq);
        $vector!(146, irq);
        $vector!(147, irq);
        $vector!(148, irq);
        $vector!(149, irq);
        $vector!(150, irq);
        $vector!(151, irq);
        $vector!(152, irq);
        $vector!(153, irq);
        $vector!(154, irq);
        $vector!(155, irq);
        $vector!(156, irq);
        $vector!(157, irq);
        $vector!(158, irq);
        $vector!(159, irq);
        $vector!(160, irq);
        $vector!(161, irq);
        $vector!(162, irq);
        $vector!(163, irq);
        $vector!(164, irq);
        $vector!(165, irq);
        $vector!(166, irq);
        $vector!(167, irq);
        $vector!(168, irq);
        $vector!(169, irq);
        $vector!(170, irq);
        $vector!(171, irq);
        $vector!(172, irq);
        $vector!(173, irq);
        $vector!(174, irq);
        $vector!(175, irq);
        $vector!(176, irq);
        $vector!(177, irq);
        $vector!(178, irq);
        $vector!(179, irq);
        $vector!(180, irq);
        $vector!(181, irq);
        $vector!(182, irq);
        $vector!(183, irq);
        $vector!(184, irq);
        $vector!(185, irq);
        $vector!(186, irq);
        $vector!(187, irq);
        $vector!(188, irq);
        $vector!(189, irq);
        $vector!(190, irq);
        $vector!(191, irq);
        $vector!(192, irq);
        $vector!(193, irq);
        $vector!(194, irq);
        $vector!(195, irq);
        $vector!(196, irq);
        $vector!(197, irq);
        $vector!(198, irq);
        $vector!(199, irq);
        $vector!(200, irq);
        $vector!(201, irq);
        $vector!(202, irq);
        $vector!(203, irq);
        $vector!(204, irq);
        $vector!(205, irq);
        $vector!(206, irq);
        $vector!(207, irq);
        $vector!(208, irq);
        $vector!(209, irq);
        $vector!(210, irq);
        $vector!(211, irq);
        $vector!(212, irq);
        $vector!(213, irq);
        $vector!(214, irq);
        $vector!(215, irq);
        $vector!(216, irq);
        $vector!(217, irq);
        $vector!(218, irq);
        $vector!(219, irq);
        $vector!(220, irq);
        $vector!(221, irq);
        $vector!(222, irq);
        $vector!(223, irq);
        $vector!(224, irq);
        $vector!(225, irq);
        $vector!(226, irq);
        $vector!(227, irq);
        $vector!(228, irq);
        $vector!(229, irq);
        $vector!(230, irq);
        $vector!(231, irq);
        $vector!(232, irq);
        $vector!(233, irq);
        $vector!(234, irq);
        $vector!(235, irq);
        $vector!(236, irq);
        $vector!(237, irq);
        $vector!(238, irq);
        $vector!(239, irq);
        $vector!(240, irq);
        $vector!(241, irq);
        $vector!(242, irq);
        $vector!(243, irq);
        $vector!(244, irq);
        $vector!(245, irq);
        $vector!(246, irq);
        $vector!(247, irq);
        $vector!(248, irq);
        $vector!(249, irq);
        $vector!(250, irq);
        $vector!(251, irq);
        $vector!(252, irq);
        $vector!(253, irq);
        $vector!(254, irq);
        $vector!(255, irq);
    };
}
