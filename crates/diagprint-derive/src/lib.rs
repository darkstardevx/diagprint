//! Procedural macros for `diagprint`.
//!
//! Runtime diagnostic behavior remains in the `diagprint` crate. This crate
//! contains only compile-time derive support.

use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Ident, LitStr, Result, parse_macro_input};

#[proc_macro_derive(Diagnostic, attributes(diag))]
pub fn derive_diagnostic(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match expand_diagnostic(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[derive(Default)]
struct TypeOptions {
    code: Option<LitStr>,
    help: Option<LitStr>,
    notes: Vec<LitStr>,
    severity: Option<SeverityValue>,
}

#[derive(Clone, Copy)]
enum SeverityValue {
    Trace,
    Debug,
    Info,
    Warning,
    Error,
    Fatal,
}

#[derive(Clone, Copy)]
enum LabelKindValue {
    Primary,
    Secondary,
}

struct LabelField {
    field: Ident,
    kind: LabelKindValue,
    message: Option<LitStr>,
    length_field: Option<Ident>,
}

fn expand_diagnostic(input: DeriveInput) -> Result<proc_macro2::TokenStream> {
    let options = parse_type_options(&input)?;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    &input,
                    "diagprint::Diagnostic currently requires a struct with named fields",
                ));
            }
        },

        _ => {
            return Err(syn::Error::new_spanned(
                &input,
                "diagprint::Diagnostic currently supports structs",
            ));
        }
    };

    let mut labels = Vec::new();

    for field in fields {
        let Some(field_ident) = field.ident.clone() else {
            continue;
        };

        if let Some(label) = parse_label_field(field, field_ident)? {
            labels.push(label);
        }
    }

    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let severity = severity_tokens(options.severity.unwrap_or(SeverityValue::Error));

    let code = match options.code {
        Some(code) => quote! {
            Some(#code.to_owned())
        },
        None => quote! {
            None
        },
    };

    let help = match options.help {
        Some(help) => quote! {
            Some(#help.to_owned())
        },
        None => quote! {
            None
        },
    };

    let notes = options.notes.iter().map(|note| {
        quote! {
            notes.push(#note.to_owned());
        }
    });

    let labels = labels.iter().map(label_tokens);

    Ok(quote! {
        impl #impl_generics ::diagprint::DiagnosticMetadata
            for #name #ty_generics #where_clause
        {
            fn diagnostic_severity(
                &self,
            ) -> ::diagprint::Severity {
                #severity
            }

            fn diagnostic_code(
                &self,
            ) -> Option<String> {
                #code
            }

            fn diagnostic_help(
                &self,
            ) -> Option<String> {
                #help
            }

            fn diagnostic_notes(
                &self,
            ) -> Vec<String> {
                let mut notes = Vec::new();
                #(#notes)*
                notes
            }

            fn diagnostic_labels(
                &self,
            ) -> Vec<::diagprint::Label> {
                let mut labels = Vec::new();
                #(#labels)*
                labels
            }
        }
    })
}

fn parse_type_options(input: &DeriveInput) -> Result<TypeOptions> {
    let mut options = TypeOptions::default();

    for attr in &input.attrs {
        if !attr.path().is_ident("diag") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("code") {
                options.code = Some(meta.value()?.parse()?);
                return Ok(());
            }

            if meta.path.is_ident("help") {
                options.help = Some(meta.value()?.parse()?);
                return Ok(());
            }

            if meta.path.is_ident("note") {
                options.notes.push(meta.value()?.parse()?);
                return Ok(());
            }

            if meta.path.is_ident("severity") {
                let severity: Ident = meta.value()?.parse()?;
                options.severity = Some(parse_severity(&severity)?);
                return Ok(());
            }

            Err(meta.error("unsupported #[diag(...)] type option"))
        })?;
    }

    Ok(options)
}

fn parse_label_field(field: &syn::Field, field_ident: Ident) -> Result<Option<LabelField>> {
    let mut kind = None;
    let mut message = None;
    let mut length_field = None;

    for attr in &field.attrs {
        if !attr.path().is_ident("diag") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("primary") {
                set_label_kind(&mut kind, LabelKindValue::Primary, &meta)?;
                return Ok(());
            }

            if meta.path.is_ident("secondary") {
                set_label_kind(&mut kind, LabelKindValue::Secondary, &meta)?;
                return Ok(());
            }

            if meta.path.is_ident("message") {
                message = Some(meta.value()?.parse()?);
                return Ok(());
            }

            if meta.path.is_ident("length") {
                length_field = Some(meta.value()?.parse()?);
                return Ok(());
            }

            Err(meta.error("unsupported #[diag(...)] field option"))
        })?;
    }

    let Some(kind) = kind else {
        return Ok(None);
    };

    Ok(Some(LabelField {
        field: field_ident,
        kind,
        message,
        length_field,
    }))
}

fn set_label_kind(
    current: &mut Option<LabelKindValue>,
    value: LabelKindValue,
    meta: &syn::meta::ParseNestedMeta<'_>,
) -> Result<()> {
    if current.is_some() {
        return Err(meta.error("a label cannot be both primary and secondary"));
    }

    *current = Some(value);

    Ok(())
}

fn parse_severity(ident: &Ident) -> Result<SeverityValue> {
    match ident.to_string().to_ascii_lowercase().as_str() {
        "trace" => Ok(SeverityValue::Trace),
        "debug" => Ok(SeverityValue::Debug),
        "info" => Ok(SeverityValue::Info),
        "warning" | "warn" => Ok(SeverityValue::Warning),
        "error" => Ok(SeverityValue::Error),
        "fatal" => Ok(SeverityValue::Fatal),

        _ => Err(syn::Error::new_spanned(
            ident,
            "severity must be trace, debug, info, warning, error, or fatal",
        )),
    }
}

fn severity_tokens(severity: SeverityValue) -> proc_macro2::TokenStream {
    match severity {
        SeverityValue::Trace => quote!(::diagprint::Severity::Trace),
        SeverityValue::Debug => quote!(::diagprint::Severity::Debug),
        SeverityValue::Info => quote!(::diagprint::Severity::Info),
        SeverityValue::Warning => quote!(::diagprint::Severity::Warning),
        SeverityValue::Error => quote!(::diagprint::Severity::Error),
        SeverityValue::Fatal => quote!(::diagprint::Severity::Fatal),
    }
}

fn label_tokens(label: &LabelField) -> proc_macro2::TokenStream {
    let field = &label.field;

    let kind = match label.kind {
        LabelKindValue::Primary => {
            quote!(::diagprint::LabelKind::Primary)
        }

        LabelKindValue::Secondary => {
            quote!(::diagprint::LabelKind::Secondary)
        }
    };

    let message = match &label.message {
        Some(message) => quote! {
            Some(#message.to_owned())
        },

        None => quote! {
            None
        },
    };

    let length = match &label.length_field {
        Some(length_field) => quote! {
            Some(self.#length_field)
        },

        None => quote! {
            None
        },
    };

    quote! {
        labels.push(::diagprint::Label {
            kind: #kind,
            location: self.#field.clone(),
            length: #length,
            message: #message,
        });
    }
}
