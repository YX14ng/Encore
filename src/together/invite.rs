//! The invite to a session: where the host is and the token that lets a guest in, written as
//! `encore:<address>:<port>:<token>`. Discord passes it along as the join secret.

use std::io::Read;

/// The bytes of randomness in a token.
const TOKEN_BYTES: usize = 16;
/// Discord's limit for a join secret.
const MAX_LENGTH: usize = 128;
const PREFIX: &str = "encore:";

/// Where to connect and with which token.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Invite {
    /// A host name, an IPv4 address or an IPv6 address in brackets.
    pub address: String,
    pub port: u16,
    pub token: String,
}

impl Invite {
    /// The invite as text, to paste or to send through Discord.
    pub fn encode(&self) -> String {
        format!("{PREFIX}{}:{}:{}", self.address, self.port, self.token)
    }

    /// Reads an invite, checking every part.
    pub fn parse(text: &str) -> Result<Self, String> {
        let text = text.trim();
        if text.len() > MAX_LENGTH {
            return Err("That invite is too long.".to_string());
        }
        let rest = text
            .strip_prefix(PREFIX)
            .ok_or("That is not an Encore invite (it starts with \"encore:\").")?;
        let mut parts = rest.rsplitn(3, ':');
        let (Some(token), Some(port), Some(address)) = (parts.next(), parts.next(), parts.next())
        else {
            return Err("That invite is incomplete.".to_string());
        };
        if !is_token(token) {
            return Err("The invite's token is broken.".to_string());
        }
        let port = port
            .parse::<u16>()
            .ok()
            .filter(|port| *port != 0)
            .ok_or("The invite's port is broken.")?;
        if !is_address(address) {
            return Err("The invite's address is broken.".to_string());
        }
        Ok(Self {
            address: address.to_string(),
            port,
            token: token.to_string(),
        })
    }

    /// Where to connect, as tokio takes it.
    pub fn socket_address(&self) -> String {
        format!("{}:{}", self.address, self.port)
    }
}

/// A new random token, from the system's random source.
pub fn new_token() -> Result<String, String> {
    let mut bytes = [0u8; TOKEN_BYTES];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut source| source.read_exact(&mut bytes))
        .map_err(|error| format!("could not read random bytes: {error}"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn is_token(token: &str) -> bool {
    token.len() == TOKEN_BYTES * 2 && token.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// A host name, an IPv4 address or a bracketed IPv6 address.
pub fn is_address(address: &str) -> bool {
    if let Some(inner) = address
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
    {
        return inner.parse::<std::net::Ipv6Addr>().is_ok();
    }
    !address.is_empty()
        && address.len() <= 253
        && !address.starts_with(['-', '.'])
        && address
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'.')
}

/// Writes an IP address the way an invite holds it (IPv6 in brackets).
pub fn address_of(ip: std::net::IpAddr) -> String {
    match ip {
        std::net::IpAddr::V4(ip) => ip.to_string(),
        std::net::IpAddr::V6(ip) => format!("[{ip}]"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";

    fn invite(address: &str) -> Invite {
        Invite {
            address: address.to_string(),
            port: 41730,
            token: TOKEN.to_string(),
        }
    }

    #[test]
    fn reads_back_what_it_writes() {
        for address in ["203.0.113.7", "home.example.org", "[2001:db8::1]"] {
            let invite = invite(address);
            assert_eq!(Invite::parse(&invite.encode()), Ok(invite));
        }
    }

    #[test]
    fn fits_in_a_discord_join_secret() {
        let longest = invite("[ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff]").encode();
        assert!(longest.len() <= MAX_LENGTH, "{} characters", longest.len());
    }

    #[test]
    fn refuses_broken_invites() {
        let token = TOKEN;
        for text in [
            "203.0.113.7:41730".to_string(),
            format!("encore:203.0.113.7:0:{token}"),
            format!("encore:203.0.113.7:99999:{token}"),
            "encore:203.0.113.7:41730:short".to_string(),
            format!("encore::41730:{token}"),
            format!("encore:-x:41730:{token}"),
            format!("encore:a b:41730:{token}"),
            format!("encore:[not-ipv6]:41730:{token}"),
            format!("encore:2001:db8::1:41730:{token}"),
        ] {
            assert!(Invite::parse(&text).is_err(), "{text} was accepted");
        }
    }

    #[test]
    fn tokens_are_random_hex() {
        let first = new_token().expect("token");
        assert!(is_token(&first));
        assert_ne!(first, new_token().expect("token"));
    }
}
