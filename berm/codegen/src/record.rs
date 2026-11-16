use berm_abi::ABI_SECTION;
use proc_macro2::TokenStream;
use quote::quote;

/// A record as JSON text, with each schema left to the type that owns it.
#[derive(Default)]
pub(crate) struct Pieces {
    pieces: Vec<TokenStream>,
    text: String,
}

impl Pieces {
    pub fn text(&mut self, text: &str) {
        self.text.push_str(text);
    }

    pub fn shape(&mut self, shape: TokenStream) {
        self.flush();
        self.pieces
            .push(quote!(::berm_lang::schema::Piece::Shape(&#shape)));
    }

    pub fn piece(&mut self, piece: TokenStream) {
        self.flush();
        self.pieces.push(piece);
    }

    pub fn array(mut self) -> TokenStream {
        self.flush();
        let pieces = self.pieces;
        quote!({
            const PIECES: &[::berm_lang::schema::Piece] = &[#(#pieces),*];
            ::berm_lang::schema::render::<{ ::berm_lang::schema::len(PIECES) }>(PIECES)
        })
    }

    pub fn emit(self) -> TokenStream {
        let array = self.array();
        quote! {
            const _: () = {
                const BYTES: &[u8] = &#array;
                #[used]
                #[cfg_attr(target_arch = "wasm32", unsafe(link_section = #ABI_SECTION))]
                static RECORD: [u8; BYTES.len()] = #array;
            };
        }
    }

    fn flush(&mut self) {
        if !self.text.is_empty() {
            let text = std::mem::take(&mut self.text);
            self.pieces
                .push(quote!(::berm_lang::schema::Piece::Text(#text)));
        }
    }
}
