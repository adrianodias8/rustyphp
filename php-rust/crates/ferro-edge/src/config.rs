//! Command-line configuration.

use std::net::IpAddr;

pub const USAGE: &str = "\
usage: ferro-edge --upstream HOST:PORT [options]
  --listen ADDR          listen address (default 0.0.0.0:8081)
  --upstream HOST:PORT   the origin: nginx+php-fpm, ferro -S, ...
  --threads N            tokio worker threads (default: available CPUs)
  --max-mb N             cache budget in MiB (default 256); FIFO eviction
  --max-object-mb N      largest cached body in MiB (default 8)
  --default-ttl S        TTL when the origin sends no max-age/s-maxage (default 0: not cached)
  --grace S              stale-while-revalidate when the origin sends none (default 10)
  --stale-if-error S     serve stale on origin failure for S seconds past TTL (default 0)
  --hit-for-pass S       remember an uncacheable URL for S seconds (default 120)
  --ban-allow CIDRS      who may BAN/PURGE/STATS, comma-separated
                         (default 127.0.0.0/8,::1/128,10.0.0.0/8,172.16.0.0/12,192.168.0.0/16)
  --session-cookies P    cookie-name prefixes that bypass the cache (default SESS,SSESS,NO_CACHE)
  --keep-cookies         forward other cookies on cacheable requests (default: strip them)
  --expose-tags          keep cache-tag headers on client responses (default: strip them)
  --stats-interval S     print counters to stderr every S seconds (default 0: off)
  --log-bans             print every BAN/PURGE and how many objects it removed";

#[derive(Clone, Debug)]
pub struct Config {
    pub listen: String,
    pub upstream: String,
    pub threads: usize,
    pub max_bytes: usize,
    pub max_object: usize,
    pub default_ttl: u64,
    pub grace: u64,
    pub stale_if_error: u64,
    pub hit_for_pass: u64,
    pub ban_allow: Vec<Cidr>,
    pub session_cookies: Vec<String>,
    pub keep_cookies: bool,
    pub expose_tags: bool,
    pub stats_interval: u64,
    pub log_bans: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            listen: "0.0.0.0:8081".into(),
            upstream: String::new(),
            threads: std::thread::available_parallelism().map_or(4, |n| n.get()),
            max_bytes: 256 << 20,
            max_object: 8 << 20,
            default_ttl: 0,
            grace: 10,
            stale_if_error: 0,
            hit_for_pass: 120,
            ban_allow: ["127.0.0.0/8", "::1/128", "10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"]
                .iter()
                .map(|c| Cidr::parse(c).unwrap())
                .collect(),
            session_cookies: vec!["SESS".into(), "SSESS".into(), "NO_CACHE".into()],
            keep_cookies: false,
            expose_tags: false,
            stats_interval: 0,
            log_bans: false,
        }
    }
}

impl Config {
    /// Parses the arguments after the program name. `Err` carries the message
    /// to print; `--help` yields `Err` with an empty message.
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Config, String> {
        let mut cfg = Config::default();
        let mut it = args.into_iter();
        while let Some(flag) = it.next() {
            let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
            match flag.as_str() {
                "-h" | "--help" => return Err(String::new()),
                "--listen" => cfg.listen = value()?,
                "--upstream" => cfg.upstream = value()?,
                "--threads" => cfg.threads = num(&flag, &value()?)? as usize,
                "--max-mb" => cfg.max_bytes = (num(&flag, &value()?)? as usize) << 20,
                "--max-object-mb" => cfg.max_object = (num(&flag, &value()?)? as usize) << 20,
                "--default-ttl" => cfg.default_ttl = num(&flag, &value()?)?,
                "--grace" => cfg.grace = num(&flag, &value()?)?,
                "--stale-if-error" => cfg.stale_if_error = num(&flag, &value()?)?,
                "--hit-for-pass" => cfg.hit_for_pass = num(&flag, &value()?)?,
                "--ban-allow" => {
                    cfg.ban_allow = value()?
                        .split(',')
                        .filter(|s| !s.trim().is_empty())
                        .map(|s| Cidr::parse(s.trim()).ok_or_else(|| format!("bad CIDR {s:?}")))
                        .collect::<Result<_, _>>()?
                }
                "--session-cookies" => {
                    cfg.session_cookies = value()?.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
                }
                "--keep-cookies" => cfg.keep_cookies = true,
                "--expose-tags" => cfg.expose_tags = true,
                "--log-bans" => cfg.log_bans = true,
                "--stats-interval" => cfg.stats_interval = num(&flag, &value()?)?,
                _ => return Err(format!("unknown option {flag}")),
            }
        }
        if cfg.upstream.is_empty() {
            return Err("--upstream is required".into());
        }
        if cfg.threads == 0 {
            return Err("--threads must be at least 1".into());
        }
        Ok(cfg)
    }

    pub fn ban_allowed(&self, ip: IpAddr) -> bool {
        let ip = ip.to_canonical();
        self.ban_allow.iter().any(|c| c.contains(ip))
    }
}

fn num(flag: &str, v: &str) -> Result<u64, String> {
    v.parse().map_err(|_| format!("{flag}: not a number: {v:?}"))
}

/// An address block, `10.0.0.0/8` or a bare address (one host).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cidr {
    net: IpAddr,
    prefix: u8,
}

impl Cidr {
    pub fn parse(s: &str) -> Option<Cidr> {
        let (addr, prefix) = match s.split_once('/') {
            Some((a, p)) => (a.parse::<IpAddr>().ok()?, Some(p.parse::<u8>().ok()?)),
            None => (s.parse::<IpAddr>().ok()?, None),
        };
        let max = if addr.is_ipv4() { 32 } else { 128 };
        let prefix = prefix.unwrap_or(max);
        (prefix <= max).then_some(Cidr { net: addr.to_canonical(), prefix })
    }

    pub fn contains(&self, ip: IpAddr) -> bool {
        match (self.net, ip.to_canonical()) {
            (IpAddr::V4(n), IpAddr::V4(a)) => {
                let mask = if self.prefix == 0 { 0 } else { u32::MAX << (32 - self.prefix) };
                u32::from(n) & mask == u32::from(a) & mask
            }
            (IpAddr::V6(n), IpAddr::V6(a)) => {
                let mask = if self.prefix == 0 { 0 } else { u128::MAX << (128 - self.prefix) };
                u128::from(n) & mask == u128::from(a) & mask
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cidr_blocks() {
        let c = Cidr::parse("172.16.0.0/12").unwrap();
        assert!(c.contains("172.20.1.2".parse().unwrap()));
        assert!(!c.contains("172.32.0.1".parse().unwrap()));
        assert!(c.contains("::ffff:172.17.0.3".parse().unwrap()));
        assert!(Cidr::parse("::1").unwrap().contains("::1".parse().unwrap()));
        assert!(Cidr::parse("0.0.0.0/0").unwrap().contains("8.8.8.8".parse().unwrap()));
        assert!(Cidr::parse("1.2.3.4/33").is_none());
    }

    #[test]
    fn parse_flags() {
        let a = |s: &str| s.split_whitespace().map(String::from).collect::<Vec<_>>();
        let c = Config::parse(a("--upstream fpm:8080 --grace 30 --ban-allow 10.0.0.0/8 --expose-tags")).unwrap();
        assert_eq!((c.upstream.as_str(), c.grace, c.ban_allow.len(), c.expose_tags), ("fpm:8080", 30, 1, true));
        assert!(Config::parse(a("--grace 3")).is_err());
        assert!(Config::parse(a("--upstream x --bogus")).is_err());
    }
}
