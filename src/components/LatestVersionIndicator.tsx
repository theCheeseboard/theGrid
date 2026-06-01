import {useEffect, useMemo, useState} from "react";

const endpoint = "https://binchicken.vicr123.com/api/repositories/thegrid?channel=stable";

interface BinChickenRelease {
    number: number,
    target: string,
    channel: string,
    version: string,
    original_filename: string
}

export function LatestVersionIndicator() {
    const [data, setData] = useState<BinChickenRelease[]>([]);

    useEffect(() => {
        (async () => {
            const response = await fetch(endpoint);
            if (!response.ok) {
                throw new Error(`Failed to fetch data: ${response.status} ${response.statusText}`);
            }
            const json = await response.json();
            setData(json);
        })();
    }, [])

    const latestVersion = useMemo(() => {
        return data[0]?.version;
    }, [data]);

    if (!latestVersion) {
        return <p>Latest version: Please wait...</p>;
    }

    return <p>Latest version: theGrid {latestVersion}</p>;
}