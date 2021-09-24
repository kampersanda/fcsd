use fcsd::*;

fn main() {
    // Sorted unique input key strings.
    let keys = [
        "deal",       // 0
        "idea",       // 1
        "ideal",      // 2
        "ideas",      // 3
        "ideology",   // 4
        "tea",        // 5
        "techie",     // 6
        "technology", // 7
        "tie",        // 8
        "trie",       // 9
    ];

    // Builds the FC string dictionary with bucket size 4.
    // Note that the bucket size needs to be a power of two.
    let dict = {
        let mut builder = FcBuilder::new(4).unwrap();
        for &key in &keys {
            builder.add(key.as_bytes()).unwrap();
        }
        FcDict::from_builder(builder)
    };

    // Locates the IDs associated with given keys.
    {
        let mut locater = FcLocater::new(&dict);
        assert_eq!(locater.run(keys[1].as_bytes()).unwrap(), 1);
        assert_eq!(locater.run(keys[7].as_bytes()).unwrap(), 7);
        assert!(locater.run("techno".as_bytes()).is_none());
    }

    // Decodes the key strings associated with given IDs.
    {
        let mut decoder = FcDecoder::new(&dict);
        assert_eq!(&decoder.run(4).unwrap(), keys[4].as_bytes());
        assert_eq!(&decoder.run(9).unwrap(), keys[9].as_bytes());
    }

    // Enumerates the stored keys and IDs in lex order.
    {
        let mut iterator = FcIterator::new(&dict);
        while let Some((id, dec)) = iterator.next() {
            assert_eq!(keys[id].as_bytes(), &dec);
        }
    }

    // Enumerates the stored keys and IDs, starting with prefix "idea", in lex order.
    {
        let mut iterator = FcPrefixIterator::new(&dict, "idea".as_bytes());
        let (id, dec) = iterator.next().unwrap();
        assert_eq!(1, id);
        assert_eq!("idea".as_bytes(), &dec);
        let (id, dec) = iterator.next().unwrap();
        assert_eq!(2, id);
        assert_eq!("ideal".as_bytes(), &dec);
        let (id, dec) = iterator.next().unwrap();
        assert_eq!(3, id);
        assert_eq!("ideas".as_bytes(), &dec);
        assert!(iterator.next().is_none());
    }

    // Serialization / Deserialization
    {
        let mut bytes = Vec::<u8>::new();
        dict.serialize_into(&mut bytes).unwrap();
        assert_eq!(bytes.len(), dict.serialized_size_in_bytes());

        let other = FcDict::deserialize_from(&bytes[..]).unwrap();
        assert_eq!(bytes.len(), other.serialized_size_in_bytes());
    }
}
