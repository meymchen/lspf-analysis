function label(count: number): string {
    return count === 1 ? "one" : "many";
}

function Badge(props: { count: number; loud: boolean }) {
    if (props.loud) {
        return <strong>{label(props.count)}</strong>;
    }
    return <span>{label(props.count)}</span>;
}
