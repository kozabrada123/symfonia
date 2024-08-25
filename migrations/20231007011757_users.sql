create table if not exists users (
    id                  numeric(20, 0)     not null constraint chk_id_range check (id >= 0 AND id <= 18446744073709551615) primary key,
    username            varchar(255)       not null,
    discriminator       varchar(255)       not null,
    avatar              varchar(255)       null,
    accent_color        int                null,
    banner              varchar(255)       null,
    theme_colors        text               null,
    pronouns            varchar(255)       null,
    phone               varchar(255)       null,
    desktop             boolean            not null default false,
    mobile              boolean            not null default false,
    premium             boolean            not null,
    premium_type        numeric(5, 0)      not null constraint chk_smallint_unsigned check (premium_type >= 0 and premium_type <= 65535),
    bot                 boolean            not null default false,
    bio                 varchar(255)       not null default '',
    system              boolean            not null default false,
    nsfw_allowed        boolean            not null default false,
    mfa_enabled         boolean            not null default false,
    webauthn_enabled    boolean            not null default false,
    totp_secret         varchar(255)       null,
    totp_last_ticket    varchar(255)       null,
    created_at          timestamp          not null,
    premium_since       timestamp          null,
    verified            boolean            not null default false,
    disabled            boolean            not null default false,
    deleted             boolean            not null default false,
    email               varchar(255)       null,
    flags               numeric(20, 0)     not null constraint chk_flags_range check (flags >= 0 AND flags <= 18446744073709551615),
    public_flags        numeric(10, 0)     not null constraint chk_int_unsigned check (public_flags >= 0 and public_flags <= 4294967295),
    purchased_flags     int                not null,
    premium_usage_flags int                not null,
    rights              numeric(20, 0)     not null constraint chk_rights_range check (rights >= 0 AND rights <= 18446744073709551615),
    data                json               not null,
    fingerprints        text               not null,
    extended_settings   json               not null,
    settings_index      numeric(20, 0)     null constraint chk_settings_index_range check (settings_index >= 0 AND settings_index <= 18446744073709551615),
    relevant_events     json               not null default '[]',
    constraint users_settings_index_uindex unique (settings_index),
    constraint users_user_settings_index_fk foreign key (settings_index) references user_settings (index)
);
