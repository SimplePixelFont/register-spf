use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create users table
        manager
            .create_table(
                Table::create()
                    .table(Users::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Users::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Users::Username)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(Users::Email)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Users::PasswordHash).string().not_null())
                    .col(
                        ColumnDef::new(Users::Role)
                            .string()
                            .not_null()
                            .default("user"),
                    )
                    .col(
                        ColumnDef::new(Users::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(AccessTokens::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AccessTokens::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(AccessTokens::UserId).integer().not_null())
                    .col(ColumnDef::new(AccessTokens::Token).string().not_null().unique_key())
                    .col(
                        ColumnDef::new(AccessTokens::TokenType)
                            .string()
                            .not_null()
                            .comment("'session' or 'api_key'"),
                    )
                    .col(
                        ColumnDef::new(AccessTokens::Name)
                            .string()
                            .comment("Name for API keys (e.g., 'CI/CD Script')"),
                    )
                    .col(
                        ColumnDef::new(AccessTokens::ExpiresAt)
                            .timestamp()
                            .comment("NULL = never expires (for API keys)"),
                    )
                    .col(ColumnDef::new(AccessTokens::LastUsedAt).timestamp())
                    .col(
                        ColumnDef::new(AccessTokens::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(AccessTokens::Table, AccessTokens::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create fonts table
        manager
            .create_table(
                Table::create()
                    .table(Fonts::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Fonts::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Fonts::Name).string().not_null())
                    .col(ColumnDef::new(Fonts::Slug).string().not_null().unique_key())
                    .col(ColumnDef::new(Fonts::Description).text())
                    .col(ColumnDef::new(Fonts::VersionNumber).integer().not_null())
                    .col(ColumnDef::new(Fonts::FilePath).string().not_null())
                    .col(ColumnDef::new(Fonts::UploadedBy).integer())
                    .col(
                        ColumnDef::new(Fonts::Status)
                            .string()
                            .not_null()
                            .default("approved"),
                    )
                    .col(
                        ColumnDef::new(Fonts::DownloadCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Fonts::FavoriteCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Fonts::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Fonts::Table, Fonts::UploadedBy)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        // Create tags table
        manager
            .create_table(
                Table::create()
                    .table(Tags::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Tags::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Tags::Name).string().not_null().unique_key())
                    .col(ColumnDef::new(Tags::Slug).string().not_null().unique_key())
                    .col(
                        ColumnDef::new(Tags::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Create font_tags junction table
        manager
            .create_table(
                Table::create()
                    .table(FontTags::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(FontTags::FontId).integer().not_null())
                    .col(ColumnDef::new(FontTags::TagId).integer().not_null())
                    .col(
                        ColumnDef::new(FontTags::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(Index::create().col(FontTags::FontId).col(FontTags::TagId))
                    .foreign_key(
                        ForeignKey::create()
                            .from(FontTags::Table, FontTags::FontId)
                            .to(Fonts::Table, Fonts::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(FontTags::Table, FontTags::TagId)
                            .to(Tags::Table, Tags::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create user_favorites table
        manager
            .create_table(
                Table::create()
                    .table(UserFavorites::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserFavorites::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(UserFavorites::UserId).integer().not_null())
                    .col(ColumnDef::new(UserFavorites::FontId).integer().not_null())
                    .col(
                        ColumnDef::new(UserFavorites::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(UserFavorites::Table, UserFavorites::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(UserFavorites::Table, UserFavorites::FontId)
                            .to(Fonts::Table, Fonts::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Comments::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Comments::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Comments::FontId).integer().not_null())
                    .col(ColumnDef::new(Comments::UserId).integer().not_null())
                    .col(ColumnDef::new(Comments::Text).text().not_null())
                    .col(
                        ColumnDef::new(Comments::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Comments::Table, Comments::FontId)
                            .to(Fonts::Table, Fonts::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Comments::Table, Comments::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create font_versions table
        manager
            .create_table(
                Table::create()
                    .table(FontVersions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FontVersions::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(FontVersions::FontId).integer().not_null())
                    .col(
                        ColumnDef::new(FontVersions::VersionNumber)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(FontVersions::FilePath).string().not_null())
                    .col(ColumnDef::new(FontVersions::Changelog).text())
                    .col(
                        ColumnDef::new(FontVersions::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(FontVersions::Table, FontVersions::FontId)
                            .to(Fonts::Table, Fonts::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create indexes for performance
        manager
            .create_index(
                Index::create()
                    .name("idx_fonts_slug")
                    .table(Fonts::Table)
                    .col(Fonts::Slug)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_users_email")
                    .table(Users::Table)
                    .col(Users::Email)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_comments_font_created")
                    .table(Comments::Table)
                    .col(Comments::FontId)
                    .col(Comments::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_comments_user")
                    .table(Comments::Table)
                    .col(Comments::UserId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_versions_font_created")
                    .table(FontVersions::Table)
                    .col(FontVersions::FontId)
                    .col(FontVersions::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tokens_token")
                    .table(AccessTokens::Table)
                    .col(AccessTokens::Token)
                    .to_owned(),
            )
            .await?;
 
        manager
            .create_index(
                Index::create()
                    .name("idx_tokens_user")
                    .table(AccessTokens::Table)
                    .col(AccessTokens::UserId)
                    .to_owned(),
            )
            .await?;
 
        manager
            .create_index(
                Index::create()
                    .name("idx_tokens_type")
                    .table(AccessTokens::Table)
                    .col(AccessTokens::TokenType)
                    .to_owned(),
            )
            .await?;

        // Unique constraint on font_id + version_number
        manager
            .create_index(
                Index::create()
                    .name("idx_versions_unique")
                    .table(FontVersions::Table)
                    .col(FontVersions::FontId)
                    .col(FontVersions::VersionNumber)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UserFavorites::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(FontTags::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Tags::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Fonts::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Users::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Comments::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(FontVersions::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(AccessTokens::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum Users {
    Table,
    Id,
    Username,
    Email,
    PasswordHash,
    Role,
    CreatedAt,
}

#[derive(Iden)]
enum AccessTokens {
    Table,
    Id,
    UserId,
    Token,
    TokenType,
    Name,
    ExpiresAt,
    LastUsedAt,
    CreatedAt,
}

#[derive(Iden)]
enum Fonts {
    Table,
    Id,
    Name,
    Slug,
    Description,
    VersionNumber,
    FilePath,
    UploadedBy,
    Status,
    DownloadCount,
    FavoriteCount,
    CreatedAt,
}

#[derive(Iden)]
enum Tags {
    Table,
    Id,
    Name,
    Slug,
    CreatedAt,
}

#[derive(Iden)]
enum FontTags {
    Table,
    FontId,
    TagId,
    CreatedAt,
}

#[derive(Iden)]
enum UserFavorites {
    Table,
    Id,
    UserId,
    FontId,
    CreatedAt,
}

#[derive(Iden)]
enum Comments {
    Table,
    Id,
    FontId,
    UserId,
    Text,
    CreatedAt,
}

#[derive(Iden)]
enum FontVersions {
    Table,
    Id,
    FontId,
    VersionNumber,
    FilePath,
    Changelog,
    CreatedAt,
}
