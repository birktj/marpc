use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Error, Parse, ParseStream, Result};
use syn::punctuated::Punctuated;
use syn::{parse_macro_input, Token};

#[derive(Clone)]
struct RpcFunc {
    vis: syn::Visibility,
    sig: RpcFuncSignature,
    block: Box<syn::Block>,
}

impl Parse for RpcFunc {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(RpcFunc {
            vis: input.parse()?,
            sig: input.parse()?,
            block: input.parse()?,
        })
    }
}

#[derive(Clone)]
struct RpcFuncSignature {
    ident: syn::Ident,
    server_args: Punctuated<RpcFuncArg, Token![,]>,
    common_args: Punctuated<RpcFuncArg, Token![,]>,
    output: syn::Type,
}

impl RpcFuncSignature {
    fn common_args_names(&self) -> Vec<syn::Ident> {
        self.common_args
            .clone()
            .into_iter()
            .map(|a| a.name)
            .collect()
    }

    fn common_args_types(&self) -> Vec<syn::Type> {
        self.common_args.clone().into_iter().map(|a| a.ty).collect()
    }

    fn server_args_names(&self) -> Vec<syn::Ident> {
        self.server_args
            .clone()
            .into_iter()
            .map(|a| a.name)
            .collect()
    }
}

impl Parse for RpcFuncSignature {
    fn parse(input: ParseStream) -> Result<Self> {
        input.parse::<Token![async]>()?;
        input.parse::<Token![fn]>()?;
        let ident = input.parse()?;

        let content;
        syn::parenthesized!(content in input);
        let args = Punctuated::<RpcFuncArg, Token![,]>::parse_terminated(&content)?;

        input.parse::<Token![->]>()?;
        let output: syn::Type = input.parse()?;

        let server_args = args.clone().into_iter().filter(|a| a.server_arg).collect();

        let common_args = args.clone().into_iter().filter(|a| !a.server_arg).collect();

        Ok(Self {
            ident: ident,
            server_args,
            common_args,
            output,
        })
    }
}

#[derive(Clone)]
struct RpcFuncArg {
    server_arg: bool,
    name: syn::Ident,
    ty: syn::Type,
}

impl Parse for RpcFuncArg {
    fn parse(input: ParseStream) -> Result<Self> {
        let server_arg = if input.peek(Token![#]) {
            input.parse::<Token![#]>()?;
            let content;
            syn::bracketed!(content in input);
            let indent = content.parse::<syn::Ident>()?;
            if indent.to_string() != "server" {
                return Err(Error::new_spanned(
                    indent,
                    "only `server` attribute supported",
                ));
            }
            if !content.is_empty() {
                return Err(Error::new(content.span(), "expected `]`"));
            }
            true
        } else {
            false
        };

        let name = input.parse()?;
        input.parse::<Token![:]>()?;
        let ty = input.parse()?;

        Ok(Self {
            server_arg,
            name,
            ty,
        })
    }
}

#[derive(Clone)]
struct RpcArgs {
    name: syn::Ident,
    uri: syn::Expr,
    service: syn::Path,
}

impl Parse for RpcArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse()?;

        let comma = input.parse::<Token![,]>()?;

        let args = Punctuated::<_, Token![,]>::parse_terminated(input)?
            .clone()
            .into_iter()
            .collect::<Vec<_>>();

        let mut uri = None;
        let mut service = None;

        for arg in args {
            match arg {
                RpcArg::Uri(x) if uri.is_some() => {
                    return Err(Error::new_spanned(x, "can only have one uri argument"))
                }
                RpcArg::Service(x) if service.is_some() => {
                    return Err(Error::new_spanned(x, "can only have one service argument"))
                }
                RpcArg::Uri(u) => uri = Some(u),
                RpcArg::Service(s) => service = Some(s),
            }
        }

        let uri = uri.ok_or(Error::new_spanned(
            comma.clone(),
            "must have an `uri` argument",
        ))?;
        let service = service.ok_or(Error::new_spanned(
            comma.clone(),
            "must have a `service` argument",
        ))?;

        Ok(Self { name, uri, service })
    }
}

#[derive(Clone)]
enum RpcArg {
    Uri(syn::Expr),
    Service(syn::Path),
}

impl Parse for RpcArg {
    fn parse(input: ParseStream) -> Result<Self> {
        let argname: syn::Ident = input.parse()?;

        match &*argname.to_string() {
            "uri" => {
                input.parse::<Token![=]>()?;
                Ok(Self::Uri(input.parse()?))
            }
            "service" => {
                input.parse::<Token![=]>()?;
                Ok(Self::Service(input.parse()?))
            }
            _ => Err(Error::new_spanned(argname, "expected `uri` or `service`")),
        }
    }
}

#[proc_macro_attribute]
pub fn rpc(attr: TokenStream, item: TokenStream) -> TokenStream {
    let RpcArgs { name, uri, service } = parse_macro_input!(attr as RpcArgs);
    let RpcFunc { vis, sig, block } = parse_macro_input!(item as RpcFunc);

    let RpcFuncSignature {
        ident,
        server_args: _,
        common_args: _,
        output,
    } = &sig;

    let common_args_names = sig.common_args_names();
    let common_args_types = sig.common_args_types();
    let server_args_names = sig.server_args_names();

    let crate_path = quote! {::marpc};

    // We want to reference the `T` in `Result<T, E>` somehow.
    // This is crazy hacky, but seems to work.
    let response_type = quote! {
        <#output as ::std::iter::IntoIterator>::Item
    };

    let rpc_method = quote! {
        #[derive(#crate_path::serde::Serialize, #crate_path::serde::Deserialize)]
        #vis struct #name {
            #(pub #common_args_names: #common_args_types),*
        }

        impl #crate_path::RpcMethod<#service> for #name {
            type Response = #response_type;
            const URI: &'static str = #uri;
        }
    };

    let server_err_type = quote! {
        <#service as #crate_path::RpcService>::ServerError
    };

    let client_err_type = quote! {
        <#service as #crate_path::ClientRpcService>::ClientError
    };

    let rpc_format = quote! {
        <<#service as #crate_path::RpcService>::Format as #crate_path::RpcFormat<#server_err_type>>
    };

    let format_err_type = quote! {
        #rpc_format::Error
    };

    let client_func_signature = {
        let err_type = quote! {
            #crate_path::ClientRpcError<#server_err_type, #client_err_type, #format_err_type>
        };

        let client_return_type = quote! {
            ::std::result::Result<#response_type, #err_type>
        };

        quote! {
            async fn #ident (#(#common_args_names: #common_args_types),*)
                -> #client_return_type
        }
    };

    let client_func = if cfg!(feature = "client") {
        quote! {
            #vis #client_func_signature {
                #crate_path::internal::rpc_call::<#service, #name>(#name {
                    #(#common_args_names),*
                }).await
            }
        }
    } else {
        quote! {}
    };

    let server_func = if cfg!(feature = "server") {
        let registry_item = quote! {
            <#service as #crate_path::internal::ServerRpcRegistry>::RegistryItem
        };

        let create_handler = quote! {
            #service::__rpc_call_internal_create_handler
        };

        let server_args = if server_args_names.is_empty() {
            quote! {_}
        } else if server_args_names.len() == 1 {
            quote! {#(#server_args_names)*}
        } else {
            quote! {(#(#server_args_names),*)}
        };

        quote! {
            #crate_path::inventory::submit!({
                static HANDLER: #registry_item = #create_handler(|| <#crate_path::internal::ServerRpcHandler<#service>>::new(#uri, |#server_args, __rpc_call_internal_method_arg| {
                    let #name {
                        #(#common_args_names),*
                    } = __rpc_call_internal_method_arg;

                    ::std::boxed::Box::pin(async move {
                        #block
                    })
                }));

                &HANDLER
            });
        }
    } else {
        quote! {}
    };

    quote! {
        #rpc_method

        #client_func

        #server_func
    }
    .into()
}
