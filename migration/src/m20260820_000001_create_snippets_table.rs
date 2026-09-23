use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Snippets::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Snippets::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Snippets::UserId).integer())
                    .col(ColumnDef::new(Snippets::Name).string().not_null())
                    .col(ColumnDef::new(Snippets::Description).text())
                    .col(ColumnDef::new(Snippets::Uuid).text().not_null())
                    .col(
                        ColumnDef::new(Snippets::Status)
                            .string()
                            .not_null()
                            .default("approved"),
                    )
                    .col(
                        ColumnDef::new(Snippets::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Snippets::Table, Snippets::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_snippets_status")
                    .table(Snippets::Table)
                    .col(Snippets::Status)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Snippets::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum Snippets {
    Table,
    Id,
    UserId,
    Name,
    Description,
    Uuid,
    Status,
    CreatedAt,
}

#[derive(Iden)]
enum Users {
    Table,
    Id,
}
