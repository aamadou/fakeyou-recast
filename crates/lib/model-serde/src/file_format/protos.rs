use bytes::BytesMut;
use prost::{DecodeError, EncodeError, Message};

pub fn encode_proto_to_bytes<T>(proto: T) -> Result<BytesMut, EncodeError>
    where T : Message
{
    let mut buffer = BytesMut::with_capacity(1024);
    proto.encode(&mut buffer)?;
    Ok(buffer)
}

pub fn decode_proto_from_bytes<T>(bytes: &[u8]) -> Result<T, DecodeError>
    where T: Message + Default
{
    // TODO: What's the cost here? Is the iterator read twice, and if so, can we reduce to one read?
    let bytes_mut = BytesMut::from_iter(bytes.iter());
    let decoded_proto = T::decode(bytes_mut)?;
    Ok(decoded_proto)
}
