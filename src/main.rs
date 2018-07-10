#![feature(plugin)]
#![feature(ip)]
#![plugin(rocket_codegen)]

extern crate chrono;
extern crate chrono_tz;
extern crate maxminddb;
extern crate rocket;

use chrono::{TimeZone, Utc};
use chrono::offset::Offset;
use chrono_tz::Tz;
use rocket::http::Status;
use rocket::{Outcome, Request};
use rocket::request::{self, FromRequest};
use std::net::IpAddr;
use std::str::FromStr;

struct RequestInfo {
    city: Option<maxminddb::geoip2::City>,
}

impl<'a, 'r> FromRequest<'a, 'r> for RequestInfo {
    type Error = ();

    fn from_request(req: &'a Request<'r>) -> request::Outcome<Self, Self::Error> {
        let remote_ip = if let Some(remote_addr) = req.remote() {
            remote_addr.ip()
        } else {
            return Outcome::Failure((Status::InternalServerError, ()));
        };

        let remote_ip = if let Some(forwarded_for) = req.headers().get_one("X-Forwarded-For") {
            let remote_ips: Vec<&'a str> = forwarded_for.split(",").collect();

            let mut remote_ip: Option<IpAddr> = None;
            for remote_ip_str in remote_ips.iter().rev() {
                let parsed_ip: Result<IpAddr, _> = FromStr::from_str(remote_ip_str.trim());
                match parsed_ip {
                    Result::Ok(ip) => remote_ip = Some(ip),
                    Result::Err(_) => {
                        return Outcome::Failure((Status::InternalServerError, ()));
                    }
                }

                if parsed_ip.unwrap().is_global() {
                    break;
                }
            }

            if remote_ip.is_none() {
                return Outcome::Failure((Status::InternalServerError, ()));
            }

            remote_ip.unwrap()
        } else {
            remote_ip
        };

        let city: Option<maxminddb::geoip2::City>;

        city = if let Ok(geoip) = maxminddb::Reader::open("./GeoLite2-City.mmdb") {
            geoip.lookup(remote_ip).unwrap_or(None)
        } else {
            None
        };

        Outcome::Success(RequestInfo { city: city })
    }
}

#[get("/timezone/offset")]
fn tzoffset(info: RequestInfo) -> Result<std::string::String, Status> {
    if info.city.is_none() {
        return Err(Status::new(500, "Server Error"));
    }

    let location = if let Some(location) = info.city.unwrap().location {
        location
    } else {
        return Err(Status::new(500, "Server Error"));
    };

    let timezone_name = if let Some(time_zone) = location.time_zone {
        time_zone
    } else {
        return Err(Status::new(500, "Server Error"));
    };

    let tz: Tz = if let Ok(tz) = timezone_name.parse() {
        tz
    } else {
        return Err(Status::new(500, "Server Error"));
    };

    let offset = tz.offset_from_utc_datetime(&Utc::now().naive_utc());

    Ok(format!("{}", offset.fix().local_minus_utc()))
}

fn main() {
    rocket::ignite().mount("/", routes![tzoffset]).launch();
}
