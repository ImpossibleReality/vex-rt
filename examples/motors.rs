#![no_std]
#![no_main]

use vex_rt::prelude::*;

struct DriveTrain {
    left_drive: Motor,
    right_drive: Motor,
}

impl DriveTrain {
    fn new(left_drive_port: SmartPort, right_drive_port: SmartPort) -> Self {
        Self {
            left_drive: left_drive_port
                .into_motor(Gearset::EighteenToOne, EncoderUnits::Degrees, true)
                .unwrap(),
            right_drive: right_drive_port
                .into_motor(Gearset::EighteenToOne, EncoderUnits::Degrees, false)
                .unwrap(),
        }
    }

    fn spin(&mut self) {
        self.left_drive.move_velocity(30).unwrap();
        self.right_drive.move_velocity(-30).unwrap();
    }

    fn stop(&mut self) {
        self.left_drive.move_velocity(0).unwrap();
        self.right_drive.move_velocity(0).unwrap();
    }
}

struct MotorsBot {
    drive_train: DriveTrain,
}

impl Robot for MotorsBot {
    fn new(peripherals: Peripherals) -> Self {
        Self {
            drive_train: DriveTrain::new(peripherals.port01, peripherals.port02),
        }
    }

    fn autonomous(&mut self, _ctx: Context) {
        println!("autonomous");
        self.drive_train.spin();
    }

    fn opcontrol(&mut self, _ctx: Context) {
        println!("opcontrol");
        self.drive_train.stop();
    }

    fn disabled(&mut self, _ctx: Context) {
        println!("disabled");
        self.drive_train.stop();
    }
}

entry!(MotorsBot);
