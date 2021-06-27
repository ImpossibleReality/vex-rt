#![no_std]
#![no_main]

use uom::si::{angular_velocity::revolution_per_minute, f64::AngularVelocity};
use vex_rt::prelude::*;

struct DriveTrain {
    left_drive: Motor,
    right_drive: Motor,
}

impl DriveTrain {
    fn new(left_drive_port: SmartPort, right_drive_port: SmartPort) -> Self {
        Self {
            left_drive: left_drive_port
                .into_motor(Gearset::EighteenToOne, true)
                .unwrap(),
            right_drive: right_drive_port
                .into_motor(Gearset::EighteenToOne, false)
                .unwrap(),
        }
    }

    fn spin(&mut self) {
        self.left_drive
            .move_velocity(AngularVelocity::new::<revolution_per_minute>(30.0))
            .unwrap();
        self.right_drive
            .move_velocity(AngularVelocity::new::<revolution_per_minute>(-30.0))
            .unwrap();
    }

    fn stop(&mut self) {
        self.left_drive
            .move_velocity(AngularVelocity::new::<revolution_per_minute>(0.0))
            .unwrap();
        self.right_drive
            .move_velocity(AngularVelocity::new::<revolution_per_minute>(0.0))
            .unwrap();
    }
}

struct ClawBot {
    drive_train: Mutex<DriveTrain>,
}

impl Robot for ClawBot {
    fn new(peripherals: Peripherals) -> Self {
        Self {
            drive_train: Mutex::new(DriveTrain::new(peripherals.port01, peripherals.port02)),
        }
    }

    fn autonomous(&'static self, _ctx: Context) {
        println!("autonomous");
        let mut drive_train = self.drive_train.lock();
        drive_train.spin();
    }

    fn opcontrol(&'static self, _ctx: Context) {
        println!("opcontrol");
        let mut drive_train = self.drive_train.lock();
        drive_train.stop();
    }

    fn disabled(&'static self, _ctx: Context) {
        println!("disabled");
        let mut drive_train = self.drive_train.lock();
        drive_train.stop();
    }
}

entry!(ClawBot);
