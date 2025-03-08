<h2>
  RPDO
  <a href="https://crates.io/crates/rpdo"><img alt="crates.io page" src="https://img.shields.io/crates/v/rpdo.svg"></img></a>
  <a href="https://docs.rs/rpdo"><img alt="docs.rs page" src="https://docs.rs/rpdo/badge.svg"></img></a>
</h2>


RoboPLC Data Objects Protocol is a lightweight fieldbus data exchange protocol,
inspired by Modbus, OPC-UA and TwinCAT/ADS.

(Work in progress)

## Locking safety

By default, the crate uses [parking_lot](https://crates.io/crates/parking_lot)
for locking. For real-time applications, the following features are available:

* `locking-rt` - use [parking_lot_rt](https://crates.io/crates/parking_lot_rt)
  crate which is a spin-free fork of parking_lot.

* `locking-rt-safe` - use [rtsc](https://crates.io/crates/rtsc)
  priority-inheritance locking, which is not affected by priority inversion
  (Linux only).

Note: to switch locking policy, disable the crate default features.

## About

RPDO is a part of [RoboPLC](https://www.roboplc.com/) project.

