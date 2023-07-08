
export async function get_pubkey() {
    return (await window.nostr.getPublicKey()).toString();
}
export async function encrypt_content(pubkey, content) {
    return (await window.nostr.nip04.encrypt(pubkey, content)).toString();
}
export async function sign_event(created_at, content, pubkey) {

    console.log(created_at);
const secondsSinceEpoch = Math.round(Date.now() / 1000)

const event = {
    created_at: secondsSinceEpoch,
    content: content,
    tags: [],
    kind: 21420,
    pubkey: pubkey,
    
};

    console.log(event);
       let e = (await window.nostr.signEvent(event));

    return JSON.stringify(e);
}
