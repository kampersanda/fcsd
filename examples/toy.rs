use fcsd::*;

fn main() {
    let keys = [
        "AirPods",     // 0
        "AirTag",      // 1
        "Mac",         // 2
        "MacBook",     // 3
        "MacBook_Air", // 4
        "MacBook_Pro", // 5
        "Mac_Mini",    // 6
        "Mac_Pro",     // 7
        "iMac",        // 8
        "iPad",        // 9
        "iPhone",      // 10
        "iPhone_SE",   // 11
    ];

    let dict = {
        let mut builder = FcBuilder::new(4).unwrap();
        for &key in &keys {
            builder.add(key.as_bytes()).unwrap();
        }
        FcDict::from_builder(builder)
    };
    {
        let mut locater = FcLocater::new(&dict);
        println!(
            "locate(Mac_Pro) = {}",
            locater.run("Mac_Pro".as_bytes()).unwrap_or(404)
        );
        println!(
            "locate(Google_Pixel) = {}",
            locater.run("Google_Pixel".as_bytes()).unwrap_or(404)
        );
    }
    {
        let mut decoder = FcDecoder::new(&dict);
        println!(
            "decode(4) = {}",
            std::str::from_utf8(decoder.run(4).unwrap()).unwrap()
        );
    }
    {
        let mut iterator = FcIterator::new(&dict);
        while let Some((id, dec)) = iterator.next() {
            println!("{} => {}", std::str::from_utf8(dec).unwrap(), id);
        }
    }
    {
        let mut iterator = FcIterator::new(&dict);
        while let Some((id, dec)) = iterator.next() {
            println!("{} => {}", std::str::from_utf8(dec).unwrap(), id);
        }
    }
    {
        let mut iterator = FcPrefixIterator::new(&dict);
        iterator.set_key("Mac".as_bytes());
        while let Some((id, dec)) = iterator.next() {
            println!("{} => {}", std::str::from_utf8(dec).unwrap(), id);
        }
    }
}
