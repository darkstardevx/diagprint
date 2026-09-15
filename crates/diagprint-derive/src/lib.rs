//! Procedural macros for `diagprint`.
//!
//! Runtime diagnostic behavior remains in the `diagprint` crate. This crate
//! contains only compile-time derive support.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Attribute, Data, DataEnum, DataStruct, DeriveInput, Fields, Ident, LitStr, Result,
    parse_macro_input,
};

#[proc_macro_derive(Diagnostic, attributes(diag))]
pub fn derive_diagnostic(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match expand_diagnostic(&input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[derive(Clone, Default)]
struct TypeOptions {
    code: Option<LitStr>,
    help: Option<LitStr>,
    notes: Vec<LitStr>,
    severity: Option<SeverityValue>,
    suggestions: Vec<SuggestionOptions>,
}

#[derive(Clone, Default)]
struct SuggestionOptions {
    title: Option<LitStr>,
    explanation: Option<LitStr>,
    applicability: Option<ApplicabilityValue>,
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum ApplicabilityValue {
    Manual,
    MaybeIncorrect,
    HasPlaceholders,
    MachineApplicable,
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

fn expand_diagnostic(input: &DeriveInput) -> Result<proc_macro2::TokenStream> {
    let options = parse_options(&input.attrs)?;

    match &input.data {
        Data::Struct(data) => expand_struct(input, data, &options),

        Data::Enum(data) => expand_enum(input, data, &options),

        Data::Union(_) => Err(syn::Error::new_spanned(
            input,
            "diagprint::Diagnostic does not support unions",
        )),
    }
}

fn expand_struct(
    input: &DeriveInput,
    data: &DataStruct,
    options: &TypeOptions,
) -> Result<proc_macro2::TokenStream> {
    let fields = match &data.fields {
        Fields::Named(fields) => &fields.named,

        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "diagprint::Diagnostic structs currently require named fields",
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

    let code = optional_string_tokens(&options.code);
    let help = optional_string_tokens(&options.help);
    let notes = notes_tokens(&options.notes);
    let suggestions = suggestions_tokens(&options.suggestions)?;

    let labels = labels.iter().map(struct_label_tokens).collect::<Vec<_>>();

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
                #notes
            }

            fn diagnostic_labels(
                &self,
            ) -> Vec<::diagprint::Label> {
                vec![#(#labels),*]
            }

            fn diagnostic_suggestions(
                &self,
            ) -> Vec<::diagprint::Suggestion> {
                #suggestions
            }
        }
    })
}

fn expand_enum(
    input: &DeriveInput,
    data: &DataEnum,
    base_options: &TypeOptions,
) -> Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let mut severity_arms = Vec::new();
    let mut code_arms = Vec::new();
    let mut help_arms = Vec::new();
    let mut notes_arms = Vec::new();
    let mut label_arms = Vec::new();
    let mut suggestion_arms = Vec::new();

    for variant in &data.variants {
        let variant_ident = &variant.ident;
        let variant_options = parse_options(&variant.attrs)?;

        let options = merge_options(base_options, &variant_options);

        let generic_pattern = match &variant.fields {
            Fields::Unit => {
                quote!(Self::#variant_ident)
            }

            Fields::Named(_) => {
                quote!(Self::#variant_ident { .. })
            }

            Fields::Unnamed(_) => {
                return Err(syn::Error::new_spanned(
                    variant,
                    "diagprint::Diagnostic enum variants currently support unit or named-field variants",
                ));
            }
        };

        let severity = severity_tokens(options.severity.unwrap_or(SeverityValue::Error));

        let code = optional_string_tokens(&options.code);

        let help = optional_string_tokens(&options.help);

        let notes = notes_tokens(&options.notes);

        let suggestions = suggestions_tokens(&options.suggestions)?;

        severity_arms.push(quote! {
            #generic_pattern => #severity,
        });

        code_arms.push(quote! {
            #generic_pattern => #code,
        });

        help_arms.push(quote! {
            #generic_pattern => #help,
        });

        notes_arms.push(quote! {
            #generic_pattern => #notes,
        });

        suggestion_arms.push(quote! {
            #generic_pattern => #suggestions,
        });

        match &variant.fields {
            Fields::Unit => {
                label_arms.push(quote! {
                    Self::#variant_ident => Vec::new(),
                });
            }

            Fields::Named(fields) => {
                let mut labels = Vec::new();

                for field in &fields.named {
                    let Some(field_ident) = field.ident.clone() else {
                        continue;
                    };

                    if let Some(label) = parse_label_field(field, field_ident)? {
                        labels.push(label);
                    }
                }

                if labels.is_empty() {
                    label_arms.push(quote! {
                        Self::#variant_ident { .. } => {
                            Vec::new()
                        },
                    });

                    continue;
                }

                let bindings = label_bindings(&labels);

                let label_values = labels.iter().map(enum_label_tokens).collect::<Vec<_>>();

                label_arms.push(quote! {
                    Self::#variant_ident {
                        #(#bindings),*,
                        ..
                    } => {
                        vec![#(#label_values),*]
                    },
                });
            }

            Fields::Unnamed(_) => unreachable!(),
        }
    }

    Ok(quote! {
        impl #impl_generics ::diagprint::DiagnosticMetadata
            for #name #ty_generics #where_clause
        {
            fn diagnostic_severity(
                &self,
            ) -> ::diagprint::Severity {
                match self {
                    #(#severity_arms)*
                }
            }

            fn diagnostic_code(
                &self,
            ) -> Option<String> {
                match self {
                    #(#code_arms)*
                }
            }

            fn diagnostic_help(
                &self,
            ) -> Option<String> {
                match self {
                    #(#help_arms)*
                }
            }

            fn diagnostic_notes(
                &self,
            ) -> Vec<String> {
                match self {
                    #(#notes_arms)*
                }
            }

            fn diagnostic_labels(
                &self,
            ) -> Vec<::diagprint::Label> {
                match self {
                    #(#label_arms)*
                }
            }

            fn diagnostic_suggestions(
                &self,
            ) -> Vec<::diagprint::Suggestion> {
                match self {
                    #(#suggestion_arms)*
                }
            }
        }
    })
}

fn parse_options(attrs: &[Attribute]) -> Result<TypeOptions> {
    let mut options = TypeOptions::default();

    for attr in attrs {
        if !attr.path().is_ident("diag") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("code") {
                if options.code.is_some() {
                    return Err(meta.error("duplicate diag code"));
                }

                options.code = Some(meta.value()?.parse()?);

                return Ok(());
            }

            if meta.path.is_ident("help") {
                if options.help.is_some() {
                    return Err(meta.error("duplicate diag help"));
                }

                options.help = Some(meta.value()?.parse()?);

                return Ok(());
            }

            if meta.path.is_ident("note") {
                options.notes.push(meta.value()?.parse()?);

                return Ok(());
            }

            if meta.path.is_ident("severity") {
                if options.severity.is_some() {
                    return Err(meta.error("duplicate diag severity"));
                }

                let severity: Ident = meta.value()?.parse()?;

                options.severity = Some(parse_severity(&severity)?);

                return Ok(());
            }

            if meta.path.is_ident("suggestion") {
                let suggestion = parse_suggestion(meta)?;

                options.suggestions.push(suggestion);

                return Ok(());
            }

            Err(meta.error("unsupported #[diag(...)] option"))
        })?;
    }

    Ok(options)
}

fn parse_suggestion(meta: syn::meta::ParseNestedMeta<'_>) -> Result<SuggestionOptions> {
    let mut suggestion = SuggestionOptions::default();

    meta.parse_nested_meta(|nested| {
        if nested.path.is_ident("title") {
            if suggestion.title.is_some() {
                return Err(nested.error("duplicate suggestion title"));
            }

            suggestion.title = Some(nested.value()?.parse()?);

            return Ok(());
        }

        if nested.path.is_ident("explanation") {
            if suggestion.explanation.is_some() {
                return Err(nested.error("duplicate suggestion explanation"));
            }

            suggestion.explanation = Some(nested.value()?.parse()?);

            return Ok(());
        }

        if nested.path.is_ident("applicability") {
            if suggestion.applicability.is_some() {
                return Err(nested.error("duplicate suggestion applicability"));
            }

            let applicability: Ident = nested.value()?.parse()?;

            suggestion.applicability = Some(parse_applicability(&applicability)?);

            return Ok(());
        }

        Err(nested.error("unsupported suggestion option"))
    })?;

    if suggestion.title.is_none() {
        return Err(meta.error("suggestion requires title = \"...\""));
    }

    if suggestion.applicability == Some(ApplicabilityValue::MachineApplicable) {
        return Err(meta.error(
            "machine_applicable suggestions require guarded edits; derive-generated edits are not supported yet",
        ));
    }

    Ok(suggestion)
}

fn merge_options(base: &TypeOptions, variant: &TypeOptions) -> TypeOptions {
    let mut notes = base.notes.clone();
    notes.extend(variant.notes.iter().cloned());

    let mut suggestions = base.suggestions.clone();
    suggestions.extend(variant.suggestions.iter().cloned());

    TypeOptions {
        code: variant.code.clone().or_else(|| base.code.clone()),

        help: variant.help.clone().or_else(|| base.help.clone()),

        notes,

        severity: variant.severity.or(base.severity),

        suggestions,
    }
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
                if message.is_some() {
                    return Err(meta.error("duplicate label message"));
                }

                message = Some(meta.value()?.parse()?);

                return Ok(());
            }

            if meta.path.is_ident("length") {
                if length_field.is_some() {
                    return Err(meta.error("duplicate label length"));
                }

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

fn label_bindings(labels: &[LabelField]) -> Vec<Ident> {
    let mut bindings = Vec::new();
    let mut names = Vec::new();

    for label in labels {
        push_binding(&mut bindings, &mut names, &label.field);

        if let Some(length_field) = &label.length_field {
            push_binding(&mut bindings, &mut names, length_field);
        }
    }

    bindings
}

fn push_binding(bindings: &mut Vec<Ident>, names: &mut Vec<String>, ident: &Ident) {
    let name = ident.to_string();

    if !names.contains(&name) {
        names.push(name);
        bindings.push(ident.clone());
    }
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

fn parse_applicability(ident: &Ident) -> Result<ApplicabilityValue> {
    match ident.to_string().to_ascii_lowercase().as_str() {
        "manual" => Ok(ApplicabilityValue::Manual),

        "maybe_incorrect" => Ok(ApplicabilityValue::MaybeIncorrect),

        "has_placeholders" => Ok(ApplicabilityValue::HasPlaceholders),

        "machine_applicable" => Ok(ApplicabilityValue::MachineApplicable),

        _ => Err(syn::Error::new_spanned(
            ident,
            "applicability must be manual, maybe_incorrect, has_placeholders, or machine_applicable",
        )),
    }
}

fn severity_tokens(severity: SeverityValue) -> proc_macro2::TokenStream {
    match severity {
        SeverityValue::Trace => {
            quote!(::diagprint::Severity::Trace)
        }

        SeverityValue::Debug => {
            quote!(::diagprint::Severity::Debug)
        }

        SeverityValue::Info => {
            quote!(::diagprint::Severity::Info)
        }

        SeverityValue::Warning => {
            quote!(::diagprint::Severity::Warning)
        }

        SeverityValue::Error => {
            quote!(::diagprint::Severity::Error)
        }

        SeverityValue::Fatal => {
            quote!(::diagprint::Severity::Fatal)
        }
    }
}

fn applicability_tokens(applicability: ApplicabilityValue) -> proc_macro2::TokenStream {
    match applicability {
        ApplicabilityValue::Manual => {
            quote!(::diagprint::Applicability::Manual)
        }

        ApplicabilityValue::MaybeIncorrect => {
            quote!(::diagprint::Applicability::MaybeIncorrect)
        }

        ApplicabilityValue::HasPlaceholders => {
            quote!(::diagprint::Applicability::HasPlaceholders)
        }

        ApplicabilityValue::MachineApplicable => {
            quote!(::diagprint::Applicability::MachineApplicable)
        }
    }
}

fn optional_string_tokens(value: &Option<LitStr>) -> proc_macro2::TokenStream {
    match value {
        Some(value) => quote! {
            Some(#value.to_owned())
        },

        None => quote! {
            None
        },
    }
}

fn notes_tokens(notes: &[LitStr]) -> proc_macro2::TokenStream {
    quote! {
        vec![#(#notes.to_owned()),*]
    }
}

fn suggestions_tokens(suggestions: &[SuggestionOptions]) -> Result<proc_macro2::TokenStream> {
    let values = suggestions
        .iter()
        .map(suggestion_tokens)
        .collect::<Result<Vec<_>>>()?;

    Ok(quote! {
        vec![#(#values),*]
    })
}

fn suggestion_tokens(suggestion: &SuggestionOptions) -> Result<proc_macro2::TokenStream> {
    let title = suggestion.title.as_ref().ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "suggestion title is required",
        )
    })?;

    let applicability = applicability_tokens(
        suggestion
            .applicability
            .unwrap_or(ApplicabilityValue::Manual),
    );

    let mut expression = quote! {
        ::diagprint::Suggestion::new(#title)
            .applicability(#applicability)
    };

    if let Some(explanation) = &suggestion.explanation {
        expression = quote! {
            #expression.explanation(#explanation)
        };
    }

    Ok(expression)
}

fn struct_label_tokens(label: &LabelField) -> proc_macro2::TokenStream {
    let field = &label.field;

    let kind = label_kind_tokens(label.kind);

    let message = optional_string_tokens(&label.message);

    let length = match &label.length_field {
        Some(length_field) => quote! {
            Some(self.#length_field)
        },

        None => quote! {
            None
        },
    };

    quote! {
        ::diagprint::Label {
            kind: #kind,
            location: self.#field.clone(),
            length: #length,
            message: #message,
        }
    }
}

fn enum_label_tokens(label: &LabelField) -> proc_macro2::TokenStream {
    let field = &label.field;

    let kind = label_kind_tokens(label.kind);

    let message = optional_string_tokens(&label.message);

    let length = match &label.length_field {
        Some(length_field) => quote! {
            Some(*#length_field)
        },

        None => quote! {
            None
        },
    };

    quote! {
        ::diagprint::Label {
            kind: #kind,
            location: (*#field).clone(),
            length: #length,
            message: #message,
        }
    }
}

fn label_kind_tokens(kind: LabelKindValue) -> proc_macro2::TokenStream {
    match kind {
        LabelKindValue::Primary => {
            quote!(::diagprint::LabelKind::Primary)
        }

        LabelKindValue::Secondary => {
            quote!(::diagprint::LabelKind::Secondary)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn machine_applicable_without_edit_is_rejected() {
        let input: DeriveInput = parse_quote! {
            #[diag(
                suggestion(
                    title = "apply fix",
                    applicability = machine_applicable
                )
            )]
            struct Example {
                location: String,
            }
        };

        let error = expand_diagnostic(&input).unwrap_err();

        assert!(error.to_string().contains("require guarded edits"));
    }

    #[test]
    fn tuple_enum_variants_are_rejected() {
        let input: DeriveInput = parse_quote! {
            enum Example {
                Unsupported(String),
            }
        };

        let error = expand_diagnostic(&input).unwrap_err();

        assert!(error.to_string().contains("unit or named-field"));
    }
}
